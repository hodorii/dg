# 가치사슬 (Value Chain) — SSoT

## Mega (가치사슬 정의서)
- id: VC-DG
- name: dg 터미널 다이어그램·마크다운 렌더링
- value: SSH·tmux 등 이미지 프로토콜이 없는 터미널 환경에서도 아키텍처·시퀀스·ERD 같은 다이어그램과
  마크다운 문서를 외부 도구 없이 그 자리에서 읽는다

## Main (주요 가치 흐름)

### 문서를 문자 그림으로 렌더링
- id: VC-DG-RENDER
- name: 문서를 문자 그림으로 렌더링
- parent: VC-DG

### 렌더링 결과를 터미널에서 탐색
- id: VC-DG-VIEW
- name: 렌더링 결과를 터미널에서 탐색
- parent: VC-DG

## Unit (단위 프로세스)

### 다이어그램 코드펜스 배치
- id: VC-DG-RENDER-DIAGRAM
- name: 다이어그램 코드펜스 배치
- parent: VC-DG-RENDER
- value: mermaid/PlantUML 다이어그램(흐름도·클래스·ERD·시퀀스·상태도·gitGraph·차트 등)을 이미지 없이
  문자 그림으로 읽는다
- validation: 지정한 폭 안에 들어가고, 안 들어가면 방향 재시도·라벨 축소 후 원문 코드블록으로 대체되며,
  어떤 입력에도 패닉하지 않는다

### 마크다운 본문 렌더링
- id: VC-DG-RENDER-MARKDOWN
- name: 마크다운 본문 렌더링
- parent: VC-DG-RENDER
- value: 제목·표·목록·강조 등 마크다운 서식을 터미널 폭에 맞게 그대로 읽는다
- validation: CommonMark/GFM 서식이 색 테마·폭 설정에 맞게 표시된다

### 페이저 탐색
- id: VC-DG-VIEW-PAGER
- name: 페이저 탐색
- parent: VC-DG-VIEW
- value: 스크롤·검색·다이어그램 원문 토글로 긴 문서를 화면 하나에서 탐색한다
- validation: 페이저 키 입력에 반응하고, 터미널 창 크기가 바뀌면 다시 렌더링된다

### 파일 변경 자동 반영
- id: VC-DG-VIEW-WATCH
- name: 파일 변경 자동 반영
- parent: VC-DG-VIEW
- value: 문서를 편집하는 동안 dg를 다시 실행하지 않고도 최신 내용을 계속 본다
- validation: 파일이 바뀌면 일정 시간 안에 다시 렌더링되고, 바뀌지 않으면 화면이 다시 그려지지
  않으며, 파일을 읽지 못해도 패닉 없이 마지막 상태를 유지한다
