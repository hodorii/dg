# BizProcess — transition_in_step(합성 fixture, 테스트 전용)

## 정의
`bizprocess-bpmn` L4 안 참여자 전환(6.5) — `activity` 정책에서 이 L4가 접히고 별도 `step` 장으로
이어지는지 확인하기 위한 합성 fixture다. 실제 `biz-process.md`가 아니다.

## L1 Process: 승인 절차 (participant: 담당자)

### L2 Activity: 검토한다
    ### L4 Step: 담당자가 내용을 확인한다
      ### L5 DetailStep: 담당자가 1차 확인한다
      ### L5 DetailStep: 승인자가 최종 승인한다 (participant: 승인자)
