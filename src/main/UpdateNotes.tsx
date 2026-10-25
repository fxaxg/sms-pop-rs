import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { notesLink } from "../shared/updater";

export function UpdateNotes({ notes, onOpen }: { notes: string; onOpen: (url: string) => void }) {
  return <div className="update-notes"><Markdown skipHtml remarkPlugins={[remarkGfm]}
    urlTransform={(url) => notesLink(url) ?? ""}
    components={{
      a: ({ href, children }) => href ? <a href={href} onClick={(event) => {
        event.preventDefault(); onOpen(href);
      }}>{children}</a> : <span>{children}</span>,
      img: ({ alt }) => <span>{alt}</span>,
    }}>{notes}</Markdown></div>;
}
