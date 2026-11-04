import { Fragment, type ReactNode } from "react";
import { api } from "./bridge";

// Tiny inline markdown: **bold**, *italic*, `code`, [text](url), bare https:// links.
const TOKEN = /(\*\*[^*]+\*\*|\*[^*\s][^*]*\*|`[^`]+`|\[[^\]]+\]\([^)]+\)|https?:\/\/[^\s)]+)/g;

export function Inline({ text }: { text: string }): ReactNode {
  const parts = text.split(TOKEN);
  return (
    <>
      {parts.map((p, i) => {
        if (!p) return null;
        if (p.startsWith("**") && p.endsWith("**")) return <strong key={i}>{p.slice(2, -2)}</strong>;
        if (p.startsWith("`") && p.endsWith("`")) return <code key={i}>{p.slice(1, -1)}</code>;
        if (p.startsWith("*") && p.endsWith("*") && p.length > 2) return <em key={i}>{p.slice(1, -1)}</em>;
        const link = /^\[([^\]]+)\]\(([^)]+)\)$/.exec(p);
        if (link) return <Link key={i} href={link[2]}>{link[1]}</Link>;
        if (/^https?:\/\//.test(p)) return <Link key={i} href={p}>{p.replace(/^https?:\/\/(www\.)?/, "").slice(0, 48)}</Link>;
        return <Fragment key={i}>{p}</Fragment>;
      })}
    </>
  );
}

export function Link({ href, children }: { href: string; children: ReactNode }) {
  return (
    <a
      href={href}
      onClick={(e) => {
        e.preventDefault();
        api.openTarget(href).catch(console.error);
      }}
    >
      {children}
    </a>
  );
}
