# Command-line changes in the native 2.1.1 executable

This file lists every deliberate difference between the 2.1.0 command line
and the native `ath`. Rule IDs refer to [command-behavior.md](command-behavior.md).
Anything not listed here keeps its 2.1.0 words, output, and exit status.
Tests compare the native executable with
[command-inventory.json](command-inventory.json).

## Deliberate changes

### 1. A refusal is printed once (quirk S4)

2.1.0 printed some refusals twice on stderr. This happened when `fail()` ran
inside an action whose `catch` called `fail()` again, as in `log`, `delete`,
`erase`, and `records show`. The native executable prints each refusal once,
as `ath: <message>`. The exit status is still 1.

Reason: the second line repeats the first and carries no information. A
reader or agent that counts error lines would misread one refusal as two.
Handlers now return the failure, and one place prints it.

### 2. `ath init` refuses when it cannot ask its questions

Without `--yes` or `--json`, `ath init` asks four profile questions on
standard input. In 2.1.0, when standard input was not a terminal, the process
stopped after a question with exit status 0 and created no file. The native
executable refuses before asking, with exit status 1:

```text
ath: there is no terminal to ask on, so nothing was created. Pass --yes to skip the questions.
```

When standard input is a terminal but ends before every question is answered
(for example, Ctrl-D at a prompt), 2.1.0 also stopped with exit status 0. The
native executable refuses with exit status 1:

```text
ath: the questions were not answered, so nothing was created. Pass --yes to skip the questions.
```

Reason: exit status 0 told a script that the database was created when it
was not. The wording follows the other non-interactive refusal, `there is no
terminal to ask on, so nothing was written. Pass --yes after reviewing the
preview.` The answers still come from standard input, as in 2.1.0.

## Version

`--version`, `-V`, and every version shown by the command line report
`2.1.1`. This follows D152's single version and is not a behavior change.
