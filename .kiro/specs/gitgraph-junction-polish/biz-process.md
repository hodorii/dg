# BizProcess — gitgraph-junction-polish

## 정의
gitGraph를 읽는 사용자를 위해, 분기·병합 연결선의 모서리를 둥글게 다듬고 세로 모드에서 커밋
id 글자와의 간격을 확보해 주는 프로세스이다.

## 가치사슬 매핑
| L1 Process | Unit Process (valueChainRef) | 가치 | 관련 요구사항 |
|------------|------------------------------|------|---------------|
| BP-GITGRAPH-JUNCTION-POLISH | VC-DG-RENDER-DIAGRAM | 이미지 없이 문자 그림만으로 다이어그램을 읽는다 | 1.1~1.6, 2.1~2.2, 3.1~3.3 |

## L1 Process: 분기·병합 지점이 다듬어진 gitGraph를 읽는다 (valueChainRef: VC-DG-RENDER-DIAGRAM)

### L2 Activity: 가로 모드에서 둥근 모서리를 본다 (1.1, 1.2, 1.6)
  ### L3 FunctionGroup/UI: 분기·병합 연결선
    ### L4 Step: 사용자가 브랜치가 갈라지고 합쳐지는 지점을 본다
      ### L5 DetailStep: 각진 모서리 대신 둥근 모서리가 보인다
        Logic(AST):
          - IF 분기 THEN 자식 트랙 쪽 모서리가 둥글게 보임 (1.1)
          - IF 병합 THEN source 트랙 쪽 모서리가 둥글게 보임 (1.2)
          - IF `--style none` THEN 색 없이도 모서리 모양 자체는 둥글게 보임 (1.6)

### L2 Activity: 세로 모드에서 둥근 모서리를 본다 (1.3, 1.4)
  ### L3 FunctionGroup/UI: 분기·병합 연결선
    ### L4 Step: 사용자가 세로 모드(`gitGraph TB:`)에서도 같은 다듬어짐을 본다
      ### L5 DetailStep: 분기·병합 모서리가 가로 모드와 다른 방향이지만(전치) 마찬가지로 둥글게
        보인다 (1.3, 1.4)

### L2 Activity: 많은 브랜치가 한 점에서 갈라지는 것도 자연스럽게 본다 (1.5)
  ### L3 FunctionGroup/UI: T자·교차 지점
    ### L4 Step: 사용자가 세 갈래 이상 갈라지는 지점을 본다
      ### L5 DetailStep: T자·교차 지점은 기존과 같은 문자로 보인다(모서리가 아니므로 둥글게
        바뀌지 않음) (1.5)

### L2 Activity: 세로 모드에서 커밋 id와 분기선이 안 붙어 보인다 (2.1, 2.2)
  ### L3 FunctionGroup/UI: 분기 연결선-커밋 id 같은 행
    ### L4 Step: 사용자가 부모의 가장 최근 커밋에서 바로 갈라지는 분기를 본다
      ### L5 DetailStep: 커밋 id 글자 뒤에 최소 한 칸 공백을 두고 분기선이 시작되는 것을 본다
        Logic(AST):
          - IF 세로 모드 + 분기가 부모의 최근 커밋 행과 같음 THEN id 글자와 연결선 사이 최소
            한 칸 공백 (2.1)
          - IF 가로 모드 THEN 기존과 동일(문제 자체가 없음) (2.2)

### L2 Activity: gitGraph 문서를 연다 (3.1, 3.2, 3.3)
  ### L3 FunctionGroup/UI: 터미널 화면
    ### L4 Step: 사용자가 gitGraph가 있는 문서를 dg로 연다
      ### L5 DetailStep: 렌더링이 정상적으로 끝난다
        Logic(AST):
          - 항상: 패닉 없이 끝남 (3.2)
          - 기존 테스트가 전량 통과함 (3.1)
          - 검증용 예제 문서(`examples/gitgraph-vertical.md`, `examples/architecture.md`)도
            패닉 없이 다시 그려짐 (3.3)

### ✅ 검토 요청 (L1: 분기·병합 지점이 다듬어진 gitGraph를 읽는다)
승인(✓) 또는 수정 사항을 입력하세요.
