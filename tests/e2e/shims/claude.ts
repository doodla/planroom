/**
 * A stand-in for the platform's `window.claude` (`artifact-capabilities` 0.2.90 `claude.d.ts`),
 * installed in the page with `page.addInitScript` after `installArtifactDb`. `use("db")` resolves
 * the fake db's page namespace, or null when the spec says the view has no db; `use("user")`
 * resolves the viewer as `user.d.ts` describes it. Like the platform, each resolves after the
 * page's first synchronous run, never during it.
 *
 * The function runs in the page, so it is self-contained.
 */

declare global {
  interface Window {
    /** The platform's `window.claude`, as far as the specs call it. */
    claude: { use(name: string): Promise<unknown> };
  }
}

export interface ClaudeShimOptions {
  /** Whether this view can run `db`; false resolves `use("db")` to null. */
  readonly db: boolean;
  /** The viewer's ID; null for a signed-out viewer. */
  readonly uid: string | null;
  readonly owner: boolean;
}

export function installClaude(options: ClaudeShimOptions): void {
  const user = Object.freeze({
    id: () => Promise.resolve(options.uid),
    isOwner: () => Promise.resolve(options.owner),
    canEdit: () => Promise.resolve(options.owner),
    can: (name: string) =>
      Promise.resolve(name === "data.write" ? options.owner : false),
    name: () => Promise.resolve(""),
    me: () =>
      Promise.resolve({
        id: options.uid,
        name: "",
        avatarUrl: "data:,",
        color: "#888888",
        email: null,
        isOwner: options.owner,
        canEdit: options.owner,
      }),
    profiles: (ids: readonly string[]) =>
      Promise.resolve(
        Object.fromEntries(
          ids.map((id) => [
            id,
            {
              id,
              name: "",
              avatarUrl: "data:,",
              color: "#888888",
              email: null,
              isMe: id === options.uid,
              guest: false,
            },
          ]),
        ),
      ),
  });
  const later = <T>(value: T) =>
    new Promise<T>((resolve) => setTimeout(() => resolve(value), 20));
  const claude = Object.freeze({
    use(name: string): Promise<unknown> {
      if (name === "db") {
        return later(options.db ? (window.__artifactDb?.page ?? null) : null);
      }
      return later(name === "user" ? user : null);
    },
  });
  Object.defineProperty(window, "claude", { value: claude });
}
