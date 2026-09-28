# BizProcess — tagged_process(합성 fixture, 테스트 전용)

## 정의
`bizprocess-bpmn` 태그·`THROW`·`ELSE IF`·L3 둘·L4 Sub-Process·이어지는 줄·괄호 이름을 한 문서에서
확인하기 위한 합성 fixture다. 실제 `biz-process.md`가 아니다. 각 Activity 장이 폭 100에 들어가도록
두 L2로 나눴다: 분류(L3 둘·태그 전환)와 심사(THROW·ELSE IF·이어지는 줄).

## L1 Process: 신청 처리 (participant: 갑)

### L2 Activity: 분류한다
  ### L3 FunctionGroup/UI: 창구(온라인)
    ### L4 Step: 갑이 접수한다
  ### L3 FunctionGroup/UI: 확인
    ### L4 Step: 을이 확인한다 (participant: 을)

### L2 Activity: 심사한다
  ### L3 FunctionGroup/UI: 심사 화면
    ### L4 Step: 갑이 서류를
      본다
      ### L5 DetailStep: 내용을 검사한다
        Logic(AST):
          - IF 정상 THEN 승인
          - ELSE IF 서류초과 THEN THROW 용량초과
          - ELSE (형식 아님) THEN 반려
