/**
 * Keeps the public Roadmap project in step with issues labeled roadmap.
 *
 * GitHub's workflow token cannot create a project on a personal account, and
 * the built-in project buttons have no API. This script uses a personal token
 * from ROADMAP_PROJECT_TOKEN. It never sets Up next or Now.
 */
import { appendFileSync, readFileSync } from "node:fs";

const STATUS = [
  { name: "Backlog", color: "GRAY", description: "Not chosen yet.", renameFrom: ["Todo"] },
  { name: "Up next", color: "YELLOW", description: "Chosen to build.", renameFrom: [] },
  { name: "Now", color: "GREEN", description: "Being built.", renameFrom: ["In Progress"] },
  { name: "Done", color: "PURPLE", description: "Finished.", renameFrom: [] },
] as const;

const LABEL = {
  name: "roadmap",
  color: "1D76DB",
  description: "A request for something to build. Starts in Backlog.",
};

const TOKEN_HELP = [
  "ROADMAP_PROJECT_TOKEN is not set.",
  "Add a personal access token as a repository secret named ROADMAP_PROJECT_TOKEN.",
  "A classic token needs the project and public_repo scopes.",
  "A fine-grained token needs account Projects read and write, and Issues read and write on this repository.",
  "The token must belong to the repository owner.",
  "Then run the Roadmap workflow from the Actions tab.",
].join(" ");

export type IssueEvent = {
  action?: string;
  label?: { name?: string };
  issue?: {
    node_id?: string;
    pull_request?: unknown;
    labels?: { name?: string }[];
  };
};

export type CardAction = { kind: "ignore" } | { kind: "add" | "set"; status: "Backlog" | "Done" };

type ExistingColumn = { id: string; name: string; color: string; description: string };
type Column = { id?: string; name: string; color: string; description: string };

/**
 * What to do with one issue event.
 * `add` sets Backlog only when the card is new. `set` always sets the status.
 */
export function cardAction(event: IssueEvent): CardAction {
  if (event.issue?.pull_request) return { kind: "ignore" };
  const names = (event.issue?.labels ?? []).flatMap((label) => (label.name ? [label.name] : []));
  if (!names.includes(LABEL.name)) return { kind: "ignore" };
  if (event.action === "closed") return { kind: "set", status: "Done" };
  if (event.action === "reopened") return { kind: "set", status: "Backlog" };
  if (event.action === "opened") return { kind: "add", status: "Backlog" };
  if (event.action === "labeled" && event.label?.name === LABEL.name) return { kind: "add", status: "Backlog" };
  return { kind: "ignore" };
}

/**
 * Four status names, reusing an existing option id when one is already there.
 * Any other column is kept. Done keeps its id so it stays the finished column.
 */
export function statusColumns(existing: ExistingColumn[]): Column[] {
  const unused = existing.map((option) => ({ ...option }));
  const take = (names: readonly string[]): ExistingColumn | null => {
    const index = unused.findIndex((option) => names.includes(option.name));
    if (index === -1) return null;
    const [taken] = unused.splice(index, 1);
    return taken ?? null;
  };
  const columns: Column[] = STATUS.map((column) => {
    const current = take([column.name]) ?? take(column.renameFrom);
    if (!current) return { name: column.name, color: column.color, description: column.description };
    return { id: current.id, name: column.name, color: column.color, description: column.description };
  });
  for (const option of unused) columns.push(option);
  return columns;
}

type RestBody = { message?: string; errors?: { code?: string }[] };

async function github(token: string, method: string, url: string, payload?: unknown): Promise<{ status: number; body: unknown }> {
  const init: RequestInit = {
    method,
    headers: {
      Authorization: `Bearer ${token}`,
      Accept: "application/vnd.github+json",
      "Content-Type": "application/json",
      "User-Agent": "athleticstandard-roadmap",
      "X-GitHub-Api-Version": "2022-11-28",
    },
  };
  if (payload !== undefined) init.body = JSON.stringify(payload);
  const response = await fetch(url, init);
  const text = await response.text();
  return { status: response.status, body: text ? JSON.parse(text) as unknown : {} };
}

async function graphql<T>(token: string, query: string, variables: Record<string, unknown>): Promise<T> {
  const { status, body } = await github(token, "POST", "https://api.github.com/graphql", { query, variables });
  const parsed = body as { data?: T; errors?: { message: string }[]; message?: string };
  if (status !== 200 || parsed.errors) {
    const message = parsed.errors?.map((error) => error.message).join("\n") || parsed.message;
    throw new Error(message || `GitHub API returned ${status}.`);
  }
  if (parsed.data === undefined) throw new Error("GitHub API returned no data.");
  return parsed.data;
}

async function ensureLabel(token: string, repo: string): Promise<void> {
  const url = `https://api.github.com/repos/${repo}/labels`;
  const created = await github(token, "POST", url, LABEL);
  if (created.status === 201) return;
  const body = created.body as RestBody;
  const exists = created.status === 422 && body.errors?.some((error) => error.code === "already_exists");
  if (!exists) throw new Error(body.message || `Could not create the roadmap label (${created.status}).`);
  const updated = await github(token, "PATCH", `${url}/${LABEL.name}`, LABEL);
  if (updated.status !== 200) {
    const updateBody = updated.body as RestBody;
    throw new Error(updateBody.message || `Could not update the roadmap label (${updated.status}).`);
  }
}

type ProjectNode = {
  id: string;
  number: number;
  title: string;
  url: string;
  repositories: { nodes: { nameWithOwner: string }[] };
};

type OwnerContext = {
  viewer: { id: string; login: string };
  repository: { id: string } | null;
  user: { projectsV2: { nodes: ProjectNode[] } } | null;
};

async function loadOwner(token: string, owner: string, name: string): Promise<OwnerContext> {
  const data = await graphql<OwnerContext>(
    token,
    `query($owner: String!, $name: String!) {
      viewer { id login }
      repository(owner: $owner, name: $name) { id }
      user(login: $owner) {
        projectsV2(first: 50, query: "Roadmap") {
          nodes {
            id
            number
            title
            url
            repositories(first: 20) { nodes { nameWithOwner } }
          }
        }
      }
    }`,
    { owner, name },
  );
  if (data.viewer.login !== owner) throw new Error("The token must belong to the repository owner.");
  if (!data.repository || !data.user) throw new Error("The repository was not found.");
  return data;
}

function chooseProject(projects: ProjectNode[], repo: string): ProjectNode | null {
  const named = projects.filter((project) => project.title === "Roadmap");
  const linked = (project: ProjectNode) => project.repositories.nodes.some((repository) => repository.nameWithOwner === repo);
  return named.find(linked) ?? named.find((project) => project.repositories.nodes.length === 0) ?? null;
}

async function createProject(token: string, ownerId: string, repositoryId: string): Promise<{ id: string; url: string }> {
  const data = await graphql<{ createProjectV2: { projectV2: { id: string; url: string } } }>(
    token,
    `mutation($ownerId: ID!, $repositoryId: ID!) {
      createProjectV2(input: { ownerId: $ownerId, title: "Roadmap", repositoryId: $repositoryId }) {
        projectV2 { id url }
      }
    }`,
    { ownerId, repositoryId },
  );
  return data.createProjectV2.projectV2;
}

async function publishProject(token: string, projectId: string): Promise<void> {
  await graphql(
    token,
    `mutation($projectId: ID!) {
      updateProjectV2(input: {
        projectId: $projectId
        title: "Roadmap"
        public: true
        shortDescription: "What is being built for Athletic Standard."
      }) {
        projectV2 { url }
      }
    }`,
    { projectId },
  );
}

function alreadyThere(error: unknown): boolean {
  return error instanceof Error && /already/i.test(error.message);
}

async function linkRepository(token: string, projectId: string, repositoryId: string, alreadyLinked: boolean): Promise<void> {
  if (alreadyLinked) return;
  try {
    await graphql(
      token,
      `mutation($projectId: ID!, $repositoryId: ID!) {
        linkProjectV2ToRepository(input: { projectId: $projectId, repositoryId: $repositoryId }) {
          repository { id }
        }
      }`,
      { projectId, repositoryId },
    );
  } catch (error) {
    if (!alreadyThere(error)) throw error;
  }
}

type StatusField = { id: string; name: string; options: ExistingColumn[] };

function isStatusField(node: { name?: string; id?: string; options?: ExistingColumn[] } | null): node is StatusField {
  return node?.name === "Status" && typeof node.id === "string" && Array.isArray(node.options);
}

async function loadStatus(token: string, projectId: string): Promise<{ url: string; field: StatusField }> {
  const data = await graphql<{
    node: { url: string; fields: { nodes: ({ name?: string; id?: string; options?: ExistingColumn[] } | null)[] } };
  }>(
    token,
    `query($id: ID!) {
      node(id: $id) {
        ... on ProjectV2 {
          url
          fields(first: 30) {
            nodes {
              ... on ProjectV2SingleSelectField {
                id
                name
                options { id name color description }
              }
            }
          }
        }
      }
    }`,
    { id: projectId },
  );
  const field = data.node.fields.nodes.find(isStatusField);
  if (!field) throw new Error("The project has no Status field.");
  return { url: data.node.url, field };
}

async function writeStatus(token: string, field: StatusField): Promise<void> {
  await graphql(
    token,
    `mutation($fieldId: ID!, $options: [ProjectV2SingleSelectFieldOptionInput!]!) {
      updateProjectV2Field(input: { fieldId: $fieldId, singleSelectOptions: $options }) {
        projectV2Field { ... on ProjectV2SingleSelectField { id } }
      }
    }`,
    { fieldId: field.id, options: statusColumns(field.options) },
  );
}

function optionId(options: ExistingColumn[], name: string): string {
  const option = options.find((item) => item.name === name);
  if (!option) throw new Error(`The Status field has no ${name} column. Run the Roadmap workflow from the Actions tab.`);
  return option.id;
}

async function itemOnProject(token: string, issueId: string, projectId: string): Promise<{ id: string } | null> {
  const data = await graphql<{ node: { projectItems: { nodes: { id: string; project: { id: string } }[] } } }>(
    token,
    `query($id: ID!) {
      node(id: $id) {
        ... on Issue {
          projectItems(first: 20) { nodes { id project { id } } }
        }
      }
    }`,
    { id: issueId },
  );
  return data.node.projectItems.nodes.find((item) => item.project.id === projectId) ?? null;
}

async function addItem(token: string, projectId: string, issueId: string): Promise<string> {
  try {
    const data = await graphql<{ addProjectV2ItemById: { item: { id: string } } }>(
      token,
      `mutation($projectId: ID!, $contentId: ID!) {
        addProjectV2ItemById(input: { projectId: $projectId, contentId: $contentId }) {
          item { id }
        }
      }`,
      { projectId, contentId: issueId },
    );
    return data.addProjectV2ItemById.item.id;
  } catch (error) {
    if (!alreadyThere(error)) throw error;
    const existing = await itemOnProject(token, issueId, projectId);
    if (!existing) throw error;
    return existing.id;
  }
}

async function setStatus(token: string, projectId: string, itemId: string, fieldId: string, option: string): Promise<void> {
  await graphql(
    token,
    `mutation($projectId: ID!, $itemId: ID!, $fieldId: ID!, $optionId: String!) {
      updateProjectV2ItemFieldValue(input: {
        projectId: $projectId
        itemId: $itemId
        fieldId: $fieldId
        value: { singleSelectOptionId: $optionId }
      }) {
        projectV2Item { id }
      }
    }`,
    { projectId, itemId, fieldId, optionId: option },
  );
}

function repositoryParts(): { owner: string; name: string; repo: string } {
  const repo = process.env.REPO;
  const parts = repo?.split("/") ?? [];
  const owner = parts[0];
  const name = parts[1];
  if (!repo || parts.length !== 2 || !owner || !name) throw new Error("REPO must be owner/name.");
  return { owner, name, repo };
}

function tokenOrNull(): string | null {
  const token = process.env.ROADMAP_PROJECT_TOKEN;
  return token ? token : null;
}

async function ensure(): Promise<void> {
  const token = tokenOrNull();
  if (!token) throw new Error(TOKEN_HELP);
  const { owner, name, repo } = repositoryParts();
  await ensureLabel(token, repo);
  const context = await loadOwner(token, owner, name);
  let project = chooseProject(context.user?.projectsV2.nodes ?? [], repo);
  const created = !project;
  if (!project) {
    if (!context.repository) throw new Error("The repository was not found.");
    project = {
      ...(await createProject(token, context.viewer.id, context.repository.id)),
      number: 0,
      title: "Roadmap",
      repositories: { nodes: [] },
    };
  }
  await publishProject(token, project.id);
  if (!context.repository) throw new Error("The repository was not found.");
  const linked = !created && project.repositories.nodes.some((repository) => repository.nameWithOwner === repo);
  await linkRepository(token, project.id, context.repository.id, linked);
  const status = await loadStatus(token, project.id);
  await writeStatus(token, status.field);
  console.log(status.url);
  if (process.env.GITHUB_STEP_SUMMARY) appendFileSync(process.env.GITHUB_STEP_SUMMARY, `Roadmap board: ${status.url}\n`);
}

async function sync(): Promise<void> {
  const token = tokenOrNull();
  if (!token) {
    console.error(TOKEN_HELP);
    return;
  }
  const path = process.env.GITHUB_EVENT_PATH;
  if (!path) throw new Error("GITHUB_EVENT_PATH is not set.");
  const event = JSON.parse(readFileSync(path, "utf8")) as IssueEvent;
  const action = cardAction(event);
  if (action.kind === "ignore") return;
  const issueId = event.issue?.node_id;
  if (!issueId) throw new Error("The issue event has no node id.");
  const { owner, name, repo } = repositoryParts();
  const context = await loadOwner(token, owner, name);
  const project = chooseProject(context.user?.projectsV2.nodes ?? [], repo);
  if (!project) {
    console.error("No Roadmap project is linked to this repository. Run the Roadmap workflow from the Actions tab.");
    return;
  }
  const status = await loadStatus(token, project.id);
  let item = await itemOnProject(token, issueId, project.id);
  const created = !item;
  if (!item) item = { id: await addItem(token, project.id, issueId) };
  if (action.kind === "add" && !created) return;
  await setStatus(token, project.id, item.id, status.field.id, optionId(status.field.options, action.status));
}

async function main(): Promise<void> {
  const command = process.argv[2];
  if (command === "ensure") await ensure();
  else if (command === "sync") await sync();
  else throw new Error("Use ensure or sync.");
}

if (process.argv[1]?.endsWith("roadmap-board.ts")) {
  main().catch((error: unknown) => {
    console.error(error instanceof Error ? error.message : error);
    process.exitCode = 1;
  });
}
