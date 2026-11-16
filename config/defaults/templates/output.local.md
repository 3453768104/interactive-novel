严格按四段格式回复。不要额外说明、不要代码块围栏、不要复述本指令。
- 正文段落里绝对不能出现 JSON 或字段名（world / player / characters / choices）；分析内容只写在 ANALYSIS 块内，正文写到 END_NARRATION 就停。
选项必须是主角此刻能做的行动／念头／对话，不替主角决定结果。
【每个选项独占一行】写成 A. … / B. … / C. …，禁止把多个选项连写在同一行。
STOP 的 stop=false 表示继续写、不必给选项；stop=true 时必须给 3 个选项。
ANALYSIS 里只写本段引起的变化，字段名照抄，值留空表示无变化。
- `summary` 只概括读者在当前视角下已经知道的事；暗场里的秘密不要写进 `summary`。

<<<NARRATION>>>
（小说正文）
<<<END_NARRATION>>>
<<<ANALYSIS>>>
{"world":{"time":"","place":"","situation":"","threads":[]},"player":{"body":"","mind":"","mood":"","goal":"","abilities":[],"items":[],"conditions":[]},"characters":[{"name":"","current_state":"","importance":"passerby","is_new":false}],"summary":"一两句话概括本段","new_character_candidates":[]}
<<<END_ANALYSIS>>>
<<<STOP>>>
{"stop":true,"reason":"为什么该停下","trigger":"命中的触发条件，没有就留空"}
<<<END_STOP>>>
<<<CHOICES>>>
A. （主角能做的一件事）
B. （主角能说的一句话）
C. （主角心里冒出的一个念头）
<<<END_CHOICES>>>
