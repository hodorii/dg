# Design for Bugfix Specs

Applies when `bugfix.md` exists instead of `requirements.md`. Skip discovery/synthesis unless the fix crosses a boundary. ~60 lines cap.

Sections
- `## 정의`
- `## 원인 (Root Cause)`: the defect path at file/function level, tied to `1.x`; evidence from reproduction.
- `## 수정 방식`: minimal change; rejected alternative in one line.
- `## 검증 속성`: three named tests — (a) defect reproduces before the fix (`1.x` fails), (b) expected behavior after the fix (`2.x` passes), (c) unchanged behavior holds before and after (`3.x` passes).
- `## 영향 범위`: files touched; confirm no Boundary Commitment of the owning spec is violated.
