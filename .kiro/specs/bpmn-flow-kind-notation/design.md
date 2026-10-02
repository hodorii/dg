# 설계서 - bpmn-flow-kind-notation

## 정의
요구사항 1~3을 캔버스 선 무늬 하나, 도형 판정 하나, BPMN 대응표와 YAML 판정 규칙으로 구현하는 설계다.

## 경계 확정

### 소속 범위 (이 스펙)
- **잔 점선 무늬**: 캔버스 네 번째 선 무늬와 곧은 칸 우선순위.
- **테두리 상자 판정과 LR 접점 분리**: 도형 판정 함수, LR 교차축 크기 규칙.
- **흐름 끝 소속 풀 판정**: 파서와 검증이 함께 쓰는 공개 규칙.
- **BPMN 대응표와 YAML 자동 선택**.
- **블랙박스 풀 면 메시지 경로의 결함 수정**(검증 중 발견, 작업 4): 풀이 도착 끝일 때 바깥 통로 방향, 한 풀 면에 메시지가 둘 이상 붙을 때 면 칸과 통로 줄 분리.

### 비소유 범위
- XML, bizprocess 흐름 종류(각 파서), 흐름 규칙 검증(`validate.rs`, 풀 판정만 공유), 풀 면 배선의 기본 규칙(면 중앙에 수직, 면 바로 바깥 통로: `bpmn-pool-lane-rendering-polish`. 이 스펙은 그 규칙 안에서 위 결함만 고친다), 공용 배치의 나머지 규칙.

### 허용 의존
- External: 없음(의존성 4개 유지).
- Internal direction: `canvas` -> `ir` -> `layout::graph` -> `bpmn::model` -> `bpmn::parse_yaml`, `bpmn::validate`, `bpmn::lower`. 역방향 가져오기는 위반.

### 재검증 조건
- `FlowKind` 증감, 새 도형 추가(판정 표 결정 필요), 접점 배치 규칙(`spread()`) 변경, 기본 폰트 가정 변경, 블랙박스 풀 메시지 배치 규칙 변경(후속 `bpmn-pool-message-layout`).

## 아키텍처

### 핵심 결정
- **곧은 칸 무늬 우선순위 대시선 > 잔 점선 > 굵은 선 > 실선**: 잔 점선이 없는 기존 출력은 바이트 동일 - reason: 4.1.
- **테두리 상자 판정은 도형으로**: 높이만으로 판정하면 이름 붙은 인터페이스(`○` + 이름, 높이 2)까지 늘어난다 - reason: 3.1의 점 모양 노드 예외.
- **풀 판정 공개화**: 검증의 비공개 판정과 같은 규칙을 파서가 써야 한다 - reason: SSoT.
- **풀 간선 표식은 대응표에서**: `draw_pool_edges()`는 간선의 `kind`, `tail`, `head`를 그대로 그리므로 1.1의 풀 면 메시지 표식은 대응표에서 따라온다.
- **풀 간선 경로 결함은 이 스펙에서 수정**(작업 4, 검증 중 발견): 풀이 도착 끝이면 통로를 풀 면 바깥에 둔다. 한 면에 끝이 둘 이상이면 면 가운데를 기준으로 칸을 나누고(상대 끝의 흐름축 순서), 통로에서 겹치는 구간은 더 바깥 줄로 보내며 모자라면 풀 사이를 넓힌다. 끝이 하나뿐인 면은 기존과 같다 - reason: 1.1(○와 빈 삼각형이 모두 보여야 함), 4.1.
- 대안과 경위는 research.md.

## 컴포넌트와 인터페이스

### canvas - 잔 점선
- 요구사항: 1.1, 4.1, 4.2
```rust
pub enum LineKind { Solid, Dashed, Heavy, Dotted }
fn line_char(bits: u8, dashed: bool, dotted: bool, heavy: bool, round: bool) -> char;
```
- 모서리, 이음, 건너뛰기 글자는 무늬와 무관(지금 규칙). 표식은 칸의 선 글자를 대신한다.

### ir - 테두리 상자 판정
- 요구사항: 3.1
```rust
impl Shape { pub fn is_border_box(self) -> bool; }
// 참: Rect Round Note Circle Subprocess Stadium Diamond Hexagon Subroutine Cylinder
```

### layout::graph - LR 접점 분리
- 요구사항: 3.1, 4.1
- 계약: LR + 점 표기 아님 + (측정 높이 3 이상, 또는 테두리 상자이고 측정 높이 2이며 max(들어옴, 나감) >= 2)이면 교차축 크기 = max(높이, 2 * max(들어옴, 나감) + 1). 그 밖은 지금 값. 흐름 수 조건은 한쪽 흐름이 하나 이하인 글자 없는 상자를 바꾸지 않기 위함이다(4.1).

### layout::graph - 블랙박스 풀 면 메시지 경로
- 요구사항: 1.1, 4.1
- 계약: `pool_edge_routes()`가 풀 간선마다 양 끝과 통로를 정하고 `draw_pool_edges()`는 그대로 그린다. 같은 풀의 같은 면에 끝이 k(>=2)개면 `spread_shared_face_points()`가 면 칸을 나누고, `channel_depths()`가 겹치는 통로 구간을 바깥 줄로 보내며, `reserve_pool_channel_room()`이 모자란 칸을 끼운다. 곧은 경로(상대 끝과 면 칸이 같은 위치)는 통로 없이 선 하나로 긋는다.

### bpmn::model - 흐름 끝 소속 풀
- 요구사항: 2.1, 2.2
```rust
impl Model { pub fn participant_of_endpoint(&self, id: &str) -> Option<usize>; }
```
- `validate.rs` `pool_of()`가 깨진 컨테이너 확인 뒤 이것을 부른다.

### bpmn::parse_yaml - 자동 선택
- 요구사항: 2.1, 2.2
```rust
fn is_flow_link(link: &Link) -> bool;
fn classify_flow(model: &Model, source: &str, target: &str) -> FlowKind; // 요구사항 2번 표
```
- 참여자와 요소로 `Model`을 먼저 만들고, 흐름을 판정한 뒤 `apply_default_flows()`. 풀 판정이 안 되는 끝은 "풀 없음"(두 끝 모두 없음이면 시퀀스).

### bpmn::lower - 대응표
- 요구사항: 1.1, 1.2
```rust
fn line_for(kind: FlowKind) -> (LineKind, Marker, Marker); // 요구사항 1번 표
```

## 오류 처리
- 세 표기 밖 화살표는 기존 `UnsupportedFlow`, 판정 뒤 규칙 위반은 기존 검증 오류로 원문 표시 - 2.1, 2.2. 풀 판정 실패는 `None`, 패닉 없음.

## 테스트 전략
- **깊이**: Standard.
- **단위**: 캔버스 잔 점선 글자와 우선순위, `line_for()` 표, `is_border_box()` 표, `participant_of_endpoint()`, `classify_flow()` 표.
- **통합**: 네 흐름 YAML의 TB, LR 렌더링(1.1, 1.2), 풀 면 메시지 양방향(1.1), 자동 선택 문서와 위반 문서(2.1, 2.2), mermaid와 BPMN 글자 없는 상자 노드, 점 모양 노드 대조(3.1).
- **인수**: `examples/bpmn.md` 실제 렌더링, 사용자 터미널에서 `┈`와 `╌` 구분 확인, `examples/architecture.txt` 차이 없음(4.1).
- **속성 기반**(저장소 안 시드 생성기):
  - P1: 글자 없는 상자 노드가 없는 그래프의 렌더링은 변경 전 기록과 같다 - 4.1 - 기존 해시 테스트 입력 공간.
  - P2: 같은 두 끝을 세 표기로 쓴 YAML의 렌더링은 같다 - 2.1 - 참여자 0~2, 노드 2~5.
  - P3: BPMN 렌더링 글자는 커버리지 허용 목록 안이다 - 4.2 - P2와 같은 공간, TB와 LR.
  - P4: LR 테두리 상자 노드에 같은 쪽 흐름 k개(2~5)는 노드 옆 칸에서 서로 다른 줄이다 - 3.1 - 도형 10종, 이름 유무.

## 파일 구조 계획
```
src/diagram/canvas.rs            code  잔 점선 무늬
src/diagram/ir.rs                code  Shape::is_border_box
src/diagram/layout/graph.rs      code  LR 교차축 크기 규칙, P4
src/diagram/layout/shape.rs      code  커버리지 허용 목록 갱신(P3)
src/diagram/bpmn/model.rs        code  participant_of_endpoint
src/diagram/bpmn/validate.rs     code  pool_of 위임
src/diagram/bpmn/parse_yaml.rs   code  is_flow_link, classify_flow, P2
src/diagram/bpmn/lower.rs        code  line_for
src/diagram/bpmn/mod.rs          code  렌더링 통합 테스트
examples/bpmn.md                 document  네 흐름 예제 블록
```
