---
title: dg 예제
---

# dg 예제 문서

**dg**는 마크다운과 *다이어그램*을 터미널에 그린다. 인라인 `code`, ~~취소~~, [링크](https://example.com)도 있다.

## 아키텍처 (PlantUML 컴포넌트)

```plantuml
@startuml
left to right direction
actor User
package "Edge" {
  [CDN] as cdn
  [API Gateway] as gw
}
package "Backend" {
  [Auth Service] as auth
  [Order Service] as order
  database "Postgres" as db
  queue "Kafka" as kafka
}
User --> cdn : HTTPS
cdn --> gw
gw --> auth : JWT 검증
gw --> order : REST
order --> db : SQL
order ..> kafka : publish
auth --> db
@enduml
```

## 흐름도 (mermaid)

```mermaid
flowchart TB
  A([시작]) --> B{로그인?}
  B -- 예 --> C[대시보드]
  B -- 아니오 --> D[로그인 폼]
  D --> E[(세션 저장)]
  E --> C
  C --> C
  C -.-> A
```

## 클래스 (mermaid)

```mermaid
classDiagram
  direction TB
  class Animal {
    +String name
    +int age
    +speak() void
  }
  class Dog {
    +fetch() void
  }
  class Cat
  <<interface>> Pet
  Animal <|-- Dog
  Animal <|-- Cat
  Pet <|.. Dog
  Dog "1" *-- "*" Toy : owns
```

## ERD (mermaid)

```mermaid
erDiagram
  CUSTOMER ||--o{ ORDER : places
  ORDER ||--|{ LINE_ITEM : contains
  PRODUCT }|--|{ LINE_ITEM : "appears in"
  CUSTOMER {
    int id PK
    string name
    string email
  }
  ORDER {
    int id PK
    date created
    int customer_id FK
  }
```

## 시퀀스 (PlantUML)

```plantuml
@startuml
autonumber
actor "사용자" as U
participant "브라우저" as B
participant "API" as A
database "DB" as D

U -> B : 로그인 클릭
B -> A ++ : POST /login
A -> D : SELECT user
D --> A : row
alt 비밀번호 일치
  A --> B -- : 200 + 토큰
  B -> B : 토큰 저장
else 불일치
  A ->x B : 401
end
note over B, A : 토큰은 메모리에만 보관
B --> U : 대시보드 표시
@enduml
```

## 상태도 (mermaid)

```mermaid
stateDiagram-v2
  [*] --> Idle
  Idle --> Running : start
  Running --> Paused : pause
  Paused --> Running : resume
  Running --> [*] : stop
```

## 표

| 항목 | 설명 | 상태 |
|------|:----:|-----:|
| 파서 | mermaid, PlantUML | 완료 |
| 배치기 | Sugiyama LR/TB | 완료 |

> 인용문은 이렇게 보인다.
> 두 줄도 된다.

- 목록 하나
- 목록 둘
  1. 번호 하나
  2. 번호 둘
- [x] 완료한 일
- [ ] 남은 일

```rust
fn main() {
    println!("hello");
}
```
