# BizProcess — <feature>

## 정의
[A]를 위해 [B]를 하는 [C]이다.

## 가치사슬 매핑
| L1 Process | Unit Process (valueChainRef) | 가치 | 관련 요구사항 |
|------------|------------------------------|------|---------------|
| BP-<x>     | VC-...-<unit>                | <value> | 1.1, 2.3 |

## L1 Process: <name>  (valueChainRef: VC-...-<unit>)
### L2 Activity: <name>  (1.1)
  ### L3 FunctionGroup/UI: <name>  (1.1, 1.2)
    ### L4 Step: <name>
      ### L5 DetailStep: <name>
        Logic(AST):
          - IF <cond> THEN <action>
          - ELSE THROW <error>
      ### L5 DetailStep: ...
    ### L4 Step: ...
### L2 Activity: ...
### ✅ 검토 요청 (L1: <name>)
승인(✓) 또는 수정 사항을 입력하세요.

## L1 Process: <name2>  (valueChainRef: VC-...-<unit2>)
... (동일 구조 반복)
### ✅ 검토 요청 (L1: <name2>)

