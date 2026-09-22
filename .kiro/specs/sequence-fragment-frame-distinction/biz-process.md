# BizProcess — sequence-fragment-frame-distinction

## 정의
시퀀스 다이어그램을 읽는 사용자를 위해, 프래그먼트 테두리를 메시지·생명선과 다른 선패턴으로
보여주는 프로세스이다.

## 가치사슬 매핑
| L1 Process | Unit Process (valueChainRef) | 가치 | 관련 요구사항 |
|------------|------------------------------|------|---------------|
| BP-SEQUENCE-FRAGMENT-FRAME-DISTINCTION | VC-DG-RENDER-DIAGRAM | 이미지 없이 문자 그림만으로 다이어그램을 읽는다 | 1.1~1.3, 2.1~2.2, 3.1~3.3 |

## L1 Process: 메시지 흐름과 구분되는 시퀀스 프래그먼트를 읽는다 (valueChainRef: VC-DG-RENDER-DIAGRAM)

### L2 Activity: 시퀀스 문서를 연다 (3.1, 3.2)
  ### L3 FunctionGroup/UI: 터미널 화면
    ### L4 Step: 사용자가 `alt`/`opt`/`loop` 등이 있는 시퀀스 문서를 dg로 연다
      ### L5 DetailStep: 렌더링이 정상적으로 끝난다
        Logic(AST):
          - 항상: 렌더링이 패닉으로 중단되지 않음 (3.2)
          - 기존 테스트가 전량 통과함 (3.1)

### L2 Activity: 프래그먼트 테두리를 흐름과 구분해서 본다 (1.1, 1.2)
  ### L3 FunctionGroup/UI: 프래그먼트 바깥 테두리
    ### L4 Step: 사용자가 굵은선 테두리를 보고 "여기서부터 여기까지가 조건/반복 블록"임을 즉시
      알아본다
      ### L5 DetailStep: 테두리가 메시지·생명선의 가는 선과 겹치지 않는 글자로 보인다
        Logic(AST):
          - IF `alt`/`opt`/`loop`/`par`/`critical`/`break`/`group`/`rect` THEN 바깥 테두리가
            굵은선으로 보임 (1.1)
          - IF `--style none` THEN 색 없이도 테두리 글자만으로 흐름과 구분됨 (1.2)

### L2 Activity: else/option/and 구분선은 기존처럼 본다 (1.3)
  ### L3 FunctionGroup/UI: 프래그먼트 내부 구분선
    ### L4 Step: 사용자가 `else`/`option`/`and`로 나뉜 대안 구간을 본다
      ### L5 DetailStep: 구분선이 기존과 동일한 파선으로 보인다 (1.3)

### L2 Activity: 화살표·생명선이 테두리를 가로지르는 지점을 본다 (2.1)
  ### L3 FunctionGroup/UI: 교차 지점
    ### L4 Step: 사용자가 메시지 화살표나 생명선이 프래그먼트 경계를 넘나드는 모습을 본다
      ### L5 DetailStep: 교차·모서리가 자연스러운 문자로 이어져 보인다 (2.1)

### L2 Activity: 중첩된 프래그먼트를 본다 (2.2)
  ### L3 FunctionGroup/UI: 중첩 프레임
    ### L4 Step: 사용자가 `loop` 안에 `alt`가 들어간 것처럼 프래그먼트가 겹친 구조를 본다
      ### L5 DetailStep: 안쪽·바깥쪽 프레임이 들여쓰기로 구분되고, 선패턴은 깊이와 무관하게
        똑같은 굵은선으로 일관되게 보인다 (2.2)

### ✅ 검토 요청 (L1: 메시지 흐름과 구분되는 시퀀스 프래그먼트를 읽는다)
승인(✓) 또는 수정 사항을 입력하세요.
