# BizProcess — markdown-gfm-alerts

## 정의
GitHub 스타일 문서를 보는 사용자를 위해, GFM 알림 블록을 종류별로 구분되는 라벨·색으로 보여주는
프로세스이다.

## 가치사슬 매핑
| L1 Process | Unit Process (valueChainRef) | 가치 | 관련 요구사항 |
|------------|------------------------------|------|---------------|
| BP-MARKDOWN-GFM-ALERTS | VC-DG-RENDER-MARKDOWN | 마크다운 서식을 터미널에서 그대로 읽는다 | 1.1~1.7, 2.1~2.2, 3.1~3.2, 4.1~4.2 |

## L1 Process: GitHub 알림 블록이 구분되어 보이는 문서를 읽는다 (valueChainRef: VC-DG-RENDER-MARKDOWN)

### L2 Activity: 알림 블록을 종류별로 구분해서 본다 (1.1~1.7, 3.1~3.2)
  ### L3 FunctionGroup/UI: 알림 블록 표시
    ### L4 Step: 사용자가 `> [!NOTE]`/`[!TIP]`/`[!IMPORTANT]`/`[!WARNING]`/`[!CAUTION]`이 있는
      문서를 dg로 연다
      ### L5 DetailStep: 알림 블록 첫 줄에서 종류를 바로 알아본다
        Logic(AST):
          - IF 다섯 종류 중 하나 THEN 종류별 라벨+색으로 표시, 원본 마커 문자열은 본문에서
            제거 (1.1~1.5, 3.1)
          - 항상: 다섯 종류가 서로 다른 색(1.6), `--style none`에서도 라벨만으로 구분(1.7)
      ### L5 DetailStep: 여러 줄·긴 줄 내용도 기존 인용문처럼 읽힌다 (3.2)

### L2 Activity: 일반 인용문은 기존처럼 본다 (2.1, 2.2)
  ### L3 FunctionGroup/UI: 일반 블록쿼트 표시
    ### L4 Step: 사용자가 마커 없는 `>` 인용문이나 인식 안 되는 `[!UNKNOWN]` 마커를 본다
      ### L5 DetailStep: 기존과 동일한 인용문 스타일로 보인다(마커 텍스트가 있으면 본문에 그대로
        남음) (2.1, 2.2)

### L2 Activity: 문서를 연다 (4.1, 4.2)
  ### L3 FunctionGroup/UI: 터미널 화면
    ### L4 Step: 사용자가 문서를 dg로 연다
      ### L5 DetailStep: 렌더링이 정상적으로 끝난다
        Logic(AST):
          - 항상: 패닉 없이 끝남 (4.1)
          - 기존 테스트가 전량 통과함 (4.2)

### ✅ 검토 요청 (L1: GitHub 알림 블록이 구분되어 보이는 문서를 읽는다)
승인(✓) 또는 수정 사항을 입력하세요.
