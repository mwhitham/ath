//! The benchmarks `ath init` defines. Athletes add their own; custom
//! benchmarks are first-class.

use crate::model::*;
use crate::store::NewRecord;

/// One seeded benchmark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeedBenchmark {
    pub key: &'static str,
    pub benchmark_type: BenchmarkType,
    pub score_type: ScoreType,
    pub definition: &'static str,
    pub tags: &'static [&'static str],
}

pub const SEED_BENCHMARKS: &[SeedBenchmark] = &[
    SeedBenchmark {
        key: "fran",
        benchmark_type: BenchmarkType::NamedWod,
        score_type: ScoreType::Time,
        definition: "21-15-9 reps for time: thrusters 95/65 lb (43/29 kg), pull-ups",
        tags: &["short", "high-power"],
    },
    SeedBenchmark {
        key: "grace",
        benchmark_type: BenchmarkType::NamedWod,
        score_type: ScoreType::Time,
        definition: "30 clean and jerks for time, 135/95 lb (61/43 kg)",
        tags: &["short", "barbell"],
    },
    SeedBenchmark {
        key: "helen",
        benchmark_type: BenchmarkType::NamedWod,
        score_type: ScoreType::Time,
        definition: "3 rounds for time: 400m run, 21 kettlebell swings 53/35 lb, 12 pull-ups",
        tags: &["medium", "mixed-modal"],
    },
    SeedBenchmark {
        key: "murph",
        benchmark_type: BenchmarkType::NamedWod,
        score_type: ScoreType::Time,
        definition: "For time: 1 mile run, 100 pull-ups, 200 push-ups, 300 air squats, 1 mile run (partition as needed)",
        tags: &["long", "endurance"],
    },
    SeedBenchmark {
        key: "5k-run",
        benchmark_type: BenchmarkType::Run,
        score_type: ScoreType::Time,
        definition: "5 kilometer run for time",
        tags: &["endurance"],
    },
    SeedBenchmark {
        key: "1-mile-run",
        benchmark_type: BenchmarkType::Run,
        score_type: ScoreType::Time,
        definition: "1 mile run for time",
        tags: &["short", "endurance"],
    },
    SeedBenchmark {
        key: "hyrox-full",
        benchmark_type: BenchmarkType::Hyrox,
        score_type: ScoreType::Time,
        definition: "8x (1km run + station): ski erg 1000m, sled push 50m, sled pull 50m, burpee broad jumps 80m, rowing 1000m, farmers carry 200m, sandbag lunges 100m, wall balls 100/75 reps",
        tags: &["long", "endurance", "stations"],
    },
];

/// The authorship calculation version of a seeded definition.
pub const SEED_DEFINITION_CALCULATION_VERSION: &str = "benchmark-definition-v1";

/// The benchmark definition records `ath init` writes, in seed order, all
/// authored by the tool at one creation time.
pub fn seed_benchmark_records(created_at: &str) -> Vec<NewRecord> {
    let tool = super::grade::tool_actor();
    SEED_BENCHMARKS
        .iter()
        .map(|seed| {
            let data = BenchmarkDefinitionData {
                authorship: AuthoredRecordMetadata {
                    author: AuthoredRecordAuthor::Tool {
                        tool: "ath".into(),
                        tool_version: ATHLETIC_STANDARD_VERSION.into(),
                        calculation_version: SEED_DEFINITION_CALCULATION_VERSION.into(),
                    },
                    authored_at: created_at.into(),
                    evidence: Vec::new(),
                    content_version: 1,
                    status: AuthoredRecordStatus::Active,
                },
                benchmark_key: seed.key.into(),
                benchmark_type: seed.benchmark_type,
                score_type: seed.score_type,
                definition: seed.definition.into(),
                tags: Some(seed.tags.iter().map(|tag| tag.to_string()).collect()),
            };
            let mut record = NewRecord::new(RecordData::BenchmarkDefinition(data), tool.clone());
            record.created_at = Some(created_at.into());
            record
        })
        .collect()
}
