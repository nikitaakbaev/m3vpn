import { useT } from "../../store/useAppStore";

interface ReleaseNotesProps {
  notes: string;
  onOpen: (url: string) => void;
}

const INLINE_PATTERN =
  /(\*\*[^*]+\*\*|\[[^\]]+\]\(https?:\/\/[^)]+\)|https?:\/\/[^\s)]+)/g;

function renderInline(text: string, onOpen: (url: string) => void) {
  const nodes: React.ReactNode[] = [];
  let last = 0;
  let index = 0;
  for (const match of text.matchAll(INLINE_PATTERN)) {
    const start = match.index ?? 0;
    if (start > last) nodes.push(text.slice(last, start));
    const token = match[0];
    if (token.startsWith("**")) {
      nodes.push(<strong key={index++}>{token.slice(2, -2)}</strong>);
    } else if (token.startsWith("[")) {
      const link = /^\[([^\]]+)\]\((https?:\/\/[^)]+)\)$/.exec(token);
      if (link) {
        nodes.push(
          <button
            key={index++}
            type="button"
            className="update-notes__link"
            onClick={() => onOpen(link[2])}
          >
            {link[1]}
          </button>,
        );
      } else {
        nodes.push(token);
      }
    } else {
      nodes.push(
        <button
          key={index++}
          type="button"
          className="update-notes__link"
          onClick={() => onOpen(token)}
        >
          {token}
        </button>,
      );
    }
    last = start + token.length;
  }
  if (last < text.length) nodes.push(text.slice(last));
  return nodes;
}

/**
 * Renders the GitHub release body as readable blocks. GitHub bodies are
 * Markdown; we intentionally support only headings, bullets, bold, links and
 * autolinks so no HTML reaches the DOM.
 */
export function ReleaseNotes({ notes, onOpen }: ReleaseNotesProps) {
  const t = useT();
  const blocks: React.ReactNode[] = [];
  let bullets: string[] = [];
  let key = 0;

  const flushBullets = () => {
    if (bullets.length === 0) return;
    blocks.push(
      <ul key={key++} className="update-notes__list">
        {bullets.map((item) => (
          <li key={key++}>{renderInline(item, onOpen)}</li>
        ))}
      </ul>,
    );
    bullets = [];
  };

  for (const rawLine of notes.split(/\r?\n/)) {
    const line = rawLine.trim().replace(/<[^>]+>/g, "");
    if (!line) {
      flushBullets();
      continue;
    }
    const bullet = /^[-*+]\s+(.*)$/.exec(line);
    if (bullet) {
      bullets.push(bullet[1]);
      continue;
    }
    flushBullets();
    const plain = line.replace(/\*\*/g, "");
    const changelog = /^Full Changelog\s*:?\s*(https?:\/\/\S+)$/i.exec(plain);
    if (changelog) {
      blocks.push(
        <button
          key={key++}
          type="button"
          className="update-notes__changelog"
          onClick={() => onOpen(changelog[1])}
        >
          {t("settings.updateChangelog")}
        </button>,
      );
      continue;
    }
    const heading = /^#{1,6}\s+(.*)$/.exec(line);
    if (heading) {
      blocks.push(
        <strong key={key++} className="update-notes__heading">
          {renderInline(heading[1], onOpen)}
        </strong>,
      );
      continue;
    }
    blocks.push(<p key={key++}>{renderInline(line, onOpen)}</p>);
  }
  flushBullets();

  return <div className="update-notes">{blocks}</div>;
}
