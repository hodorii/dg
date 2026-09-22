# BizProcess — gitgraph-branch-distinction

## 정의
gitGraph 문서를 읽는 사용자를 위해, 트랙(브랜치)별로 색상과 선패턴이 자동 배정된 화면을 보여주는
프로세스이다.

## 가치사슬 매핑
| L1 Process | Unit Process (valueChainRef) | 가치 | 관련 요구사항 |
|------------|------------------------------|------|---------------|
| BP-GITGRAPH-BRANCH-DISTINCTION | VC-DG-RENDER-DIAGRAM | 이미지 없이 문자 그림만으로 다이어그램을 읽는다 | 1.1~1.3, 2.1~2.2, 3.1~3.2, 4.1~4.2, 5.1~5.2 |

## L1 Process: 브랜치별로 구분되는 gitGraph를 읽는다 (valueChainRef: VC-DG-RENDER-DIAGRAM)

### L2 Activity: gitGraph 문서를 연다 (5.1, 5.2)
  ### L3 FunctionGroup/UI: 터미널 화면
    ### L4 Step: 사용자가 gitGraph 코드펜스가 있는 마크다운 문서를 dg로 연다
      ### L5 DetailStep: 렌더링된 트랙 수를 본다
        Logic(AST):
          - IF 트랙 수 == 1 THEN 기존과 동일한 단일 실선으로 표시됨 (5.1)
          - ELSE 트랙별 색+선패턴 구분이 적용됨
          - 항상: 렌더링이 패닉으로 중단되지 않음 (5.2)

### L2 Activity: 색상으로 브랜치를 구분한다 (1.1, 1.2, 1.3)
  ### L3 FunctionGroup/UI: 트랙 선·커밋 점·브랜치 이름 라벨
    ### L4 Step: 사용자가 트랙마다 다른 색을 비교해 브랜치를 식별한다
      ### L5 DetailStep: 트랙 선·커밋 점·이름 라벨이 같은 색으로 묶여 보인다 (1.1)
      ### L5 DetailStep: 트랙 수가 색 종류보다 많으면 앞에서부터 재사용된 색을 본다 (1.2)
      ### L5 DetailStep: 세로 모드(TB/BT)로 봐도 같은 색 규칙을 본다 (1.3)
        Logic(AST):
          - IF 트랙 순번 < 색 종류 수 THEN 순번 그대로의 색
          - ELSE 순번 % 색 종류 수 번째 색으로 재사용

### L2 Activity: 선패턴으로 브랜치를 구분한다 (2.1, 2.2)
  ### L3 FunctionGroup/UI: 트랙 선
    ### L4 Step: 사용자가 실선/파선/굵은선 등 선패턴 차이로 브랜치를 식별한다
      ### L5 DetailStep: 색이 비슷해 보여도 선패턴으로 구분된다 (2.1)
      ### L5 DetailStep: 트랙이 하나뿐이면 기존과 같은 단일 실선을 본다 (2.2)

### L2 Activity: `--style none`에서도 브랜치를 구분한다 (3.1, 3.2)
  ### L3 FunctionGroup/UI: 색상 없는 터미널 출력
    ### L4 Step: 사용자가 색 없이 선패턴만으로 트랙을 구분한다
      ### L5 DetailStep: 트랙마다 다른 선패턴이 그대로 보인다 (3.1)
        Logic(AST):
          - IF `--style none` THEN 색상 출력 생략, 선패턴만 표시
          - ELSE 색+선패턴 모두 표시
      ### L5 DetailStep: 트랙 수가 선패턴 종류보다 많으면 앞에서부터 재사용된 패턴을 본다 (3.2)

### L2 Activity: 분기·병합 지점에서 브랜치 흐름을 따라간다 (4.1, 4.2)
  ### L3 FunctionGroup/UI: 분기·병합 연결선, 병합 커밋 점
    ### L4 Step: 사용자가 연결선의 스타일을 보고 어느 브랜치에서 갈라지거나 합쳐지는지 인지한다
      ### L5 DetailStep: 분기선이 새로 생기는 트랙의 색·선패턴으로 보인다 (4.1)
      ### L5 DetailStep: 병합선이 합류해 들어오는 트랙의 색·선패턴으로 보이고, 병합 커밋 점은
        받는 쪽 트랙의 색으로 보인다 (4.2)

### ✅ 검토 요청 (L1: 브랜치별로 구분되는 gitGraph를 읽는다)
승인(✓) 또는 수정 사항을 입력하세요.
