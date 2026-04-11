use criterion::{Criterion, criterion_group, criterion_main};
use llm_transpiler::{FidelityLevel, InputFormat, transpile};

const SAMPLE_MD: &str = r#"
# 소프트웨어 라이선스 계약

## 계약 당사자

본 계약은 갑(라이선서)과 을(라이선시) 사이에 체결됩니다.

## 주요 조항

- 소스 코드 배포 금지
- 역설계 금지
- 연간 라이선스 비용: 1,000,000원

| 항목 | 금액 |
|------|------|
| 기본료 | 800,000원 |
| 유지보수 | 200,000원 |
"#;

fn bench_transpile_semantic(c: &mut Criterion) {
    c.bench_function("transpile_semantic_4096", |b| {
        b.iter(|| {
            transpile(
                SAMPLE_MD,
                InputFormat::Markdown,
                FidelityLevel::Semantic,
                Some(4096),
            )
            .unwrap()
        })
    });
}

fn bench_transpile_lossless(c: &mut Criterion) {
    c.bench_function("transpile_lossless", |b| {
        b.iter(|| {
            transpile(
                SAMPLE_MD,
                InputFormat::Markdown,
                FidelityLevel::Lossless,
                None,
            )
            .unwrap()
        })
    });
}

criterion_group!(benches, bench_transpile_semantic, bench_transpile_lossless);
criterion_main!(benches);
