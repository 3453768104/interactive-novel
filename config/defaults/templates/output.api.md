请只返回一个 JSON 对象，不要代码块围栏，不要任何额外说明。字段如下：
- 正文段落里绝对不能出现 JSON 或字段名（world / player / characters / choices）；分析内容只写在 ANALYSIS 块内，正文写到 END_NARRATION 就停。

{"narration":"小说正文","analysis":{"world":{"time":"","place":"","situation":"","threads":[]},"player":{"body":"","mind":"","mood":"","goal":"","abilities":[],"items":[],"conditions":[]},"characters":[{"name":"","current_state":"","importance":"passerby","is_new":false}],"summary":"一两句话概括本段","new_character_candidates":[]},"stop":{"stop":true,"reason":"为什么该停下","trigger":"命中的触发条件"},"choices":["A. ...","B. ...","C. ..."]}

要求：
- choices 必须是主角此刻能做的行动／念头／对话，不替主角决定结果，不写作者视角的剧情走向。
- 【重要】choices 数组的每个元素只放一个选项；禁止把多个选项连写进同一个字符串（例如 `"A. … B. … C. …"`）。
- stop.stop=false 表示接着写，此时 choices 可为空数组；stop.stop=true 时至少给 3 个选项。
- analysis 只写本段引起的变化，字段名照抄，值留空表示无变化。
- analysis.characters[].importance 只能是 passerby / notable；新出场角色把 is_new 设为 true。
