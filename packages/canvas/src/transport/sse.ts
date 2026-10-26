/**
 * Server-sent events read through `fetch`, so the bearer token rides in a header (DR-29): an
 * incremental parser over the text the stream yields.
 */

/** One SSE message. */
export interface SseMessage {
  readonly event: string;
  readonly id: string | null;
  readonly data: string;
}

/** Feed it chunks; it returns the messages each chunk completes. */
export class SseParser {
  private buffer = "";
  private event = "";
  private data: string[] = [];
  private id: string | null = null;
  /** The last `id:` seen, for `Last-Event-ID` on a resume. */
  lastEventId: string | null = null;

  push(chunk: string): SseMessage[] {
    this.buffer += chunk;
    const out: SseMessage[] = [];
    for (;;) {
      const match = /\r\n|\r|\n/.exec(this.buffer);
      // A lone \r at the end may be the first half of \r\n: wait for more.
      if (
        !match ||
        (match[0] === "\r" && match.index === this.buffer.length - 1)
      ) {
        break;
      }
      const line = this.buffer.slice(0, match.index);
      this.buffer = this.buffer.slice(match.index + match[0].length);
      const message = this.line(line);
      if (message) {
        out.push(message);
      }
    }
    return out;
  }

  private line(line: string): SseMessage | null {
    if (line === "") {
      if (this.data.length === 0) {
        this.event = "";
        return null;
      }
      const message = {
        event: this.event || "message",
        id: this.id,
        data: this.data.join("\n"),
      };
      this.event = "";
      this.data = [];
      return message;
    }
    if (line.startsWith(":")) {
      return null;
    }
    const colon = line.indexOf(":");
    const field = colon < 0 ? line : line.slice(0, colon);
    let value = colon < 0 ? "" : line.slice(colon + 1);
    if (value.startsWith(" ")) {
      value = value.slice(1);
    }
    switch (field) {
      case "event":
        this.event = value;
        break;
      case "data":
        this.data.push(value);
        break;
      case "id":
        if (!value.includes("\0")) {
          this.id = value;
          this.lastEventId = value;
        }
        break;
      default:
        break;
    }
    return null;
  }
}
