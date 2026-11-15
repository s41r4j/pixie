# skills/

Playbooks the job agent reads before it starts on something they cover. Each is
`skills/<name>/SKILL.md` with front matter:

```markdown
---
name: pdf
description: one line that says when to use it
---
```

The app lists every skill (name, description, path) in the prompt's environment block. Users can
add their own in `~/Library/Application Support/pixie.s41r4j.in/skills/`.

These playbooks are Pixie's own. MIT-licensed playbooks from other projects (e.g.
NousResearch/hermes-agent) can be dropped in with their attribution kept.
