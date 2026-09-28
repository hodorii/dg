# Research — layout-crossgroup-edge-blowup

## Summary
- **Feature**: `layout-crossgroup-edge-blowup`
- **Discovery Scope**: Extension (기존 배치 코드의 성능 결함)
- **Key Findings**:
  - 폭발은 `improve_by_swaps`가 아니라 `Layout::build()`의 접기 루프에서 일어난다 — 그룹 블록
    접기가 폭을 줄이지 못하면서(또는 줄이면서도) 층을 누적시키고, 층에 비례해 가상 노드·구간이
    늘며, 배치 한 번의 비용이 구간 수의 2차다.
  - 접기 규칙을 바꾸는 모든 후보는 표본 248건에서 결과 퇴행(그림 → 폴백 또는 라벨 폭 단계
    변경)을 냈다. 결과를 바꾸지 않는 색인화 (c)(d)(b) + 절대 구간 상한 (a)만이 퇴행 0·타임아웃
    0·최대 0.27초를 동시에 만족했다.
  - 1차 검토(외부 적대적 검토)가 지적한 대로 r1(층 가중 정체)은 5.5% 퇴행이 실측됐고,
    "다항식" 일반 주장은 D가 깊이에 비례하는 경로에서 성립하지 않는다.

## Research Log

### 어느 단계가 시간을 쓰는가 (계측)
- **Sources**: `DG_DEBUG=1` 로그(기존), 임시 `Instant` 계측(`arrange` 단계별, `resolve_port_swaps`
  내부, `apply_fold` 후보, `improve_by_swaps` 후보 평가) — 측정 후 전부 원복(`git diff -- src/`
  없음). `perf`는 `perf_event_paranoid=4`로 불가. 계측 빌드는 LTO 없음·debuginfo라 절대값이
  release보다 약 1.2~1.5배 크다.
- **Findings**(실사용 펜스, 계측 빌드, 수정 전): 배치 68회 합계 3.59초 = `route` 1.14 +
  `reserve_label_room` 뒤 2차 패스 1.46 + `ports/straighten` 0.82 + `place/reorder` 0.16;
  `resolve_port_swaps` 합 1.92초(54%); `improve_by_swaps` 0.
- 접기 추적(실사용 펜스): `fold target=block members=6 row_layer=11 others_max=19` → `row_layer=20
  others_max=28` → … 6구성원 블록이 자기 층 범위 바로 아래로 9층씩, 폭 559 불변.
- seed 2·N=20: 층 21→42→63→121→…→881(+80/접기), 구간 → 12,576, `resolve_port_swaps` 한 번
  7.5초(회차 3, 같은 층 쌍 519,456, 가상 노드 교환 32,595회). seed 17·N=30: 폭이 줄어드는
  접기가 층을 45→90→180→280→382로(접기당 ×1.4~2).
- (c)(d)(b) 뒤 잔여(계측 빌드): 실사용 펜스 배치 68회 0.68초 → `place/reorder` 0.20,
  `straighten` 0.17, 2차 패스 0.20, `route` 0.07. `improve_by_swaps` 후보 평가 1회 ≈ 2ms
  (chain_s9_c30: 214회 0.48초, 루트 자식 413) → (d) 뒤 최대 0.39초.

### 표본 (bugfix.md 재현 4·6)
248건 = 기본형 24 + 기본형 확장 80 + 허브·사슬·라벨형 144. 수정 전(release, 12초 타임아웃,
8병렬): 그림 128 / 폴백 19 / 타임아웃 101, 합계 1,559초. 그림 128건의 높이 중앙값 337행(최대
619행), 60건이 1초 초과. 폴백↔타임아웃 분할은 문턱 근처에서 실행마다 흔들리므로 비교 기준은
"수정 전 그림 128건의 바이트"와 "시간 상한".

### 규칙 후보 비교 (계측 빌드, 같은 표본; `퇴행` = 그림 → 폴백, `변경` = 그림 바이트 변경)
| 규칙 | 퇴행 | 변경 | 타임아웃 | 합계 시간 | 비고 |
|---|---|---|---|---|---|
| 수정 전 | – | – | 101 | 1,559초 | 기준 |
| r1: 폭 못 줄인 접기의 `stalls += 늘어난 층 수`, 예산 12 | 7 | 3 | 1~9 | 227~558초 | 퇴행 7건은 모두 수정 전 337~619행·1.1~8.9초 그림 |
| r5: r1 + 예산 `max(12, L₀)` | 3 | 3 | 8 | 610초 | |
| r6: r1 + 예산 `max(12, 2·L₀)` | 0 | 3 | 11 | 609초 | 변경 3건 = 더 좁은 라벨 단계에서 그림 |
| s3/s4/s6: 구간 수 ≤ 초기의 3/4/6배 | 5/2/0 | 4/3/1 | 2/8/26 | 430/636/842초 | 상대 상한은 초기 구간 수가 작은 입력을 조기에 끊는다 |
| a1500: 구간 수 ≤ 1,500 (절대) | 0 | 0 | 4 | 614초 | 수정 전 그림의 최종 구간 수 최대 992 |
| r1 + a1500 | 7 | 3 | 0 | 232초 | |
| r1 + 이웃 교환 건너뛰기(구간 > 300 / 400) | 7 | 52 / 34 | 0~1 | 153~174초 | 안전판은 결과를 크게 바꾼다 |
| (b)(c)만, 접기 규칙 그대로 | 0 | 0 | 37 | 899초 | 상한 없이는 폭주가 남는다 |
| **a1500 + (b)(c)** | 0 | 0 | 0 | 110초 | 최대 3.23초(8병렬) |
| **a1500 + (b)(c)(d)** — 채택 | 0 | 0 | 0 | 109초 | 최대 1.53초(8병렬), 순차 실행 최대 0.27초(release) |

- r1의 퇴행 7건(base_s15_c20, base_s16_c15, base_s6_c15, base_s8_c25, hub_s6_c30, labels_s0_c10,
  labels_s8_c10)은 첫 접기가 폭을 늘리며 층을 12개 넘게 늘려 예산을 즉시 소진하지만 수정 전
  코드는 1~2번 더 접어 폭에 들어갔던 경우(최종 구간 228~900). 변경 3건(labels_s7/s12/s13_c10)은
  cap 30 단계에서 포기해 cap 22에서 그린 경우.
- release 프로파일 시험 빌드(a1500 + (b)(c)(d)) 순차 실측: 실사용 펜스 0.18초(수정 전 2.58),
  문서 0.22초(1.7~2.9), seed 2·N=20 0.06초(60초 초과), cross_15 0.01초, 가장 느린 10건
  0.14~0.27초. 결과 바이트: 그림 128건·`examples/*.md` 8개 × 폭 120/200(16건) 전부 동일.

### `improve_by_swaps` 잔여
- 수정 전 표본(r1 하): 그림 121건 중 44건이 이웃 교환에만 1초 초과, 최대 21.6초(구간 700+).
- (c) 뒤 최대 2.0초(base_s8_c20, 후보 295회, 루트 자식 364 — 후보 평가 1회 6~7ms, `straighten_layer`
  의 `children.clone()` N·C 항과 층 필터 L·N 항이 지배). (d) 뒤 최대 0.39초, 중앙값 0.02초.
- 건너뛰기 안전판(구간 300/400 초과)은 그림 52/34건의 결과를 바꿔 기각. 후보 평가 수 예산은 결과를
  바꾸므로 이 스펙 밖의 후속 결정.

### 배치당 비용의 동치 변환 근거
- `resolve_port_swaps`: 원 루프는 i 오름차순 × j 오름차순에서 층이 다르면 건너뜀. j를 "i와 같은
  층 버킷(색인 오름차순)"으로 바꾸면 방문 쌍·순서 동일. 교환 뒤 전체 `refresh_ports()`는
  (교환된 두 가상 노드에 닿는 구간) ∪ (직전 갱신 이후 nudge된 구간)만 갱신하는 것과 같다 —
  나머지 구간의 입력(`across`·오프셋)이 안 바뀌었기 때문. 시험 구현으로 표본·예제 전부 바이트 동일.
- `straighten_layer`·`assign_ports`: 노드별 구간 목록(색인 순)은 원래 `segments.iter().filter(...)`와
  같은 원소·순서; 층별 노드 목록(색인 순) → `sort_by_key(across)`(안정)라 같은 결과; 레인 자식
  목록은 레인 자식이 있는 블록을 `reorder()`·`improve_by_swaps()`가 건너뛰어 순서가 고정.
- `count_crossings`는 이미 층별 버킷(1ec8845) — 같은 패턴의 선례.

## Design Decisions

### Decision: 접기 규칙은 건드리지 않고 절대 구간 상한만 둔다(즉시 접기, patience 없음)
- **Context**: 접기 규칙 후보(r1/r5/r6, 상대 상한)는 모두 표본에서 결과를 바꿨다.
- **Selected**: `FOLD_SEGMENT_LIMIT = 1500` 절대 상한(한 번만 넘어도 즉시 접는다) + 결과 동일
  색인화. 상한은 비용의 직접 원인(구간 수)에 걸고, 수정 전 코드가 **12초 타임아웃 안에** 완료한
  어떤 그림(최대 992)도 끊지 않는다.
- **Trade-offs**: 실사용 펜스처럼 상한 아래에서 쳇바퀴를 다 도는 입력은 여전히 배치 56~68회를
  한다(단, 회당 ≈ 3ms). 상한을 넘는 입력은 수정 전엔 (수십 초 뒤) 폴백이거나 1,000행 가까운
  그림이었다 — **Revalidation trigger 발동, 이번에 확인됨(아래)**.

### Revalidation: 상한 1,500 근처의 표본 밖 입력 — 실제로 발견됨
- **경위**: 표본 248건 자체는 12초 타임아웃·8병렬로 걸러 만들어졌는데, 이 경계 밖(수정 전
  12초를 넘겨야 끝나는 입력)에서 실제로 "상한을 넘겼다가 회복해 그림으로 끝나는" 사례가
  독립 검토에서 발견됐다: `labels_s3_c20`(수정 전 15초, 접기 도중 구간 수 2712→4158→4577,
  즉시-접기라면 결과가 바뀐다), `base_s9_c40`(수정 전 447초, streak 7에서 회복, 그림 1823줄),
  `labels_s17_c15`, `base_s10_c25`, `labels_s17_c25`, `hub_s7_c40`.
- **왜 patience로 못 고치나**: streak(연속으로 상한을 넘긴 접기 횟수) 기준 회복 분포를
  480+320건 스윕(무제한 patience 계측)으로 재면 {0: 270건, 2: 1, 3: 3, 4: 1, 7: 1건
  (`base_s9_c40`)}인데, **streak 8~17까지 가도 회복하지 않는 진짜 쳇바퀴가 10건** 같은
  분포 안에 섞여 있다. 즉 "몇 회 참을지" 값 하나로 "곧 회복"과 "영원히 회복 안 함"을 가를
  수 없다 — patience=4(가장 작은 반례 `labels_s3_c20`만 구제)를 시험해도 480건 중 39건이
  1초를 넘고(최대 7.57초) 배치 크기가 상한의 11배(16,716구간)까지 부푼다.
- **결론**: 즉시-접기 원안을 유지한다. 대신 bugfix.md 2.4/design.md에 "12초 밖 입력은 결과가
  바뀔 수 있다(허용)"를 명시해 Boundary Context를 정직하게 좁혔다 — 이게 이 Revalidation의
  해소 방식이다.

### Decision: 이웃 교환 안전판은 넣지 않고 "알려진 한계"로 명시
- 건너뛰기는 결과를 바꾸고(52/34건), (c)(d) 뒤 잔여 최대 0.39초라 2.2의 표본 약속 안에 든다.

## Risks & Mitigations
- (b)의 국소 갱신이 nudge 구간을 빠뜨리거나 `from`/`to` 한쪽만 갱신하는 실수 — 골든 diff(표본
  128 + 예제 16)로 검출; 불변식 ①~③을 코드 주석으로. (구현 완료 후 독립 검토가 `resolve_port_
  swaps()`·`assign_ports()`·`straighten_layer()` 세 곳 모두에 원본-대조 실행을 붙여 392/392
  실물 A/B로 재확인 — 결과 동일성은 확정.)
- 상한 1,500 근처의 표본 밖 입력 — Revalidation trigger 발동, 위에서 해소(즉시-접기 유지 +
  Boundary Context 명시).
- 후속(범위 밖): `improve_by_swaps` 후보 평가 수 예산, `place_block` Θ(C²)·`route` Θ(L·S)·
  `reorder` Θ(N·C) 색인화(모두 결과 동일 변환 후보), `reserve_label_room` 2차 패스.

## References
- `src/diagram/layout/graph.rs` — `Layout::build`, `apply_fold`, `make_segments`, `build_blocks`,
  `resolve_port_swaps`, `straighten_layer`, `assign_ports`, `improve_by_swaps`, `count_crossings`.
- 커밋 1ec8845(`LARGE_GRAPH_NODES`·`count_crossings` 층별 버킷), 5d4ca37(접기 루프·
  `FOLD_PATIENCE`), 6ba9408(`resolve_port_swaps`).
- 호출처: `src/diagram/mermaid/mod.rs`(flowchart·state·er·class), `src/diagram/plantuml/mod.rs`
  (3곳), `src/diagram/bpmn/mod.rs`(2곳). 그룹 생성: `mermaid/class.rs`·`mermaid/state.rs`·
  `plantuml/component.rs`·`plantuml/class.rs`·`bpmn/lower.rs`.
