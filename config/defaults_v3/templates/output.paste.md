【最重要的要求】
请严格按下面四段的格式回复，整块回复会被原样复制回应用解析。
不要添加任何额外说明、客套话、开场白、结束语、总结，也不要使用代码块围栏（不要写 ```）。
选项必须是主角此刻能做的行动／念头／对话，不替主角决定结果。
【每个选项独占一行】写成 A. … / B. … / C. …，禁止把多个选项连写在同一行。
STOP 的 stop=false 表示继续写、不必给选项；stop=true 时必须给 3 个选项。
ANALYSIS 里只写本段引起的变化，字段名照抄，值留空表示无变化。

【必须使用的格式】
<<<NARRATION>>>
（这里写小说正文）
<<<END_NARRATION>>>
<<<ANALYSIS>>>
{"world":{"time":"","place":"","situation":"","threads":[]},"player":{"body":"","mind":"","items":[],"goal":""},"characters":[{"name":"","current_state":"","importance":"passerby","is_new":false}],"summary":"一两句话概括本段","new_character_candidates":[]}
<<<END_ANALYSIS>>>
<<<STOP>>>
{"stop":true,"reason":"为什么该停下","trigger":"命中的触发条件，没有就留空"}
<<<END_STOP>>>
<<<CHOICES>>>
A. （主角能做的一件事）
B. （主角能说的一句话）
C. （主角心里冒出的一个念头）
<<<END_CHOICES>>>
