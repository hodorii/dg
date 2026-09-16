graph TD
    cli_rs_H7900af90["cli.rs — 1 mod, 0 fn, 1 struct"]
    diagram["diagram — 20 mod, 195 fn, 20 struct"]
    line_rs_Hc8ff652c["line.rs — 1 mod, 17 fn, 3 struct"]
    main_rs_H9177877a["main.rs — 1 mod, 7 fn, 0 struct"]
    markdown["markdown — 3 mod, 24 fn, 6 struct"]
    pager_rs_H4b0ff4c1["pager.rs — 1 mod, 13 fn, 2 struct"]
    style_rs_Hd51fafb9["style.rs — 1 mod, 13 fn, 2 struct"]
    text_rs_H94e4f82b["text.rs — 1 mod, 5 fn, 0 struct"]
    diagram -->|"4 calls"| line_rs_Hc8ff652c
    diagram -->|"16 calls"| text_rs_H94e4f82b
    line_rs_Hc8ff652c -->|"2 calls"| text_rs_H94e4f82b
    main_rs_H9177877a -->|"2 calls"| diagram
    main_rs_H9177877a -->|"1 calls"| markdown
    main_rs_H9177877a -->|"3 calls"| style_rs_Hd51fafb9
    markdown -->|"2 calls"| diagram
    markdown -->|"12 calls"| line_rs_Hc8ff652c
    markdown -->|"5 calls"| text_rs_H94e4f82b
    pager_rs_H4b0ff4c1 -->|"3 calls"| line_rs_Hc8ff652c
    pager_rs_H4b0ff4c1 -->|"1 calls"| markdown
    pager_rs_H4b0ff4c1 -->|"2 calls"| text_rs_H94e4f82b
%% analyzed 29 files, 388 atoms, 152 cross-module edges
