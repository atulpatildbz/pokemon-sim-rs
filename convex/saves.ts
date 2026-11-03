// Synced files of the web build (see web/cloud.js): save slots under /data/saves/.
import { mutation, query } from "./_generated/server";
import { v } from "convex/values";
import { allowedUser, requireUser } from "./auth";

const PATH = /^\/data\/saves\/[A-Za-z0-9_-]{1,64}\.pgs$/;
const MAX_BYTES = 256 * 1024;

/** Who is signed in (null: nobody, or an account that is not allowed). */
export const me = query({
  args: {},
  handler: async (ctx) => {
    const user = await allowedUser(ctx);
    return user ? { email: user.email ?? "", name: user.name ?? "" } : null;
  },
});

/** Every synced file's path and version (no contents): cheap to watch for other devices' changes. */
export const list = query({
  args: {},
  handler: async (ctx) => {
    const user = await allowedUser(ctx);
    if (!user) return null;
    const rows = await ctx.db.query("files").withIndex("by_user_path", (q) => q.eq("userId", user._id)).collect();
    return rows.map((r) => ({ path: r.path, modified: r.modified, device: r.device, deleted: r.data === null }));
  },
});

/** Every synced file with its contents (read once at start-up). */
export const pull = query({
  args: {},
  handler: async (ctx) => {
    const user = await requireUser(ctx);
    const rows = await ctx.db.query("files").withIndex("by_user_path", (q) => q.eq("userId", user._id)).collect();
    return rows.map((r) => ({ path: r.path, modified: r.modified, device: r.device, data: r.data }));
  },
});

/** Store a version of a file (`data: null` deletes it). An older version than the stored one is
 *  ignored: returns false. */
export const push = mutation({
  args: { path: v.string(), data: v.union(v.bytes(), v.null()), modified: v.number(), device: v.string() },
  handler: async (ctx, { path, data, modified, device }) => {
    const user = await requireUser(ctx);
    if (!PATH.test(path)) throw new Error(`Not a synced path: ${path}`);
    if (data !== null && data.byteLength > MAX_BYTES) throw new Error(`${path} is too big to sync`);
    const existing = await ctx.db
      .query("files")
      .withIndex("by_user_path", (q) => q.eq("userId", user._id).eq("path", path))
      .first();
    const row = { userId: user._id, path, data, modified, device, updatedAt: Date.now() };
    if (!existing) {
      await ctx.db.insert("files", row);
      return true;
    }
    if (existing.modified > modified) return false;
    await ctx.db.replace(existing._id, row);
    return true;
  },
});
