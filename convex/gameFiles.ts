// The player's own PokeGame.exe / POKéGAME.dat, kept in file storage so another device can
// start without them. Only signed-in, allowlisted accounts can read or replace them.
import { mutation, query } from "./_generated/server";
import { v } from "convex/values";
import { requireUser } from "./auth";

export const get = query({
  args: {},
  handler: async (ctx) => {
    await requireUser(ctx);
    const rows = await ctx.db.query("gameFiles").collect();
    const out: { name: "exe" | "dat"; size: number; url: string }[] = [];
    for (const r of rows) {
      const url = await ctx.storage.getUrl(r.storageId);
      if (url) out.push({ name: r.name, size: r.size, url });
    }
    return out;
  },
});

export const uploadUrl = mutation({
  args: {},
  handler: async (ctx) => {
    await requireUser(ctx);
    return await ctx.storage.generateUploadUrl();
  },
});

export const register = mutation({
  args: { name: v.union(v.literal("exe"), v.literal("dat")), storageId: v.id("_storage") },
  handler: async (ctx, { name, storageId }) => {
    const user = await requireUser(ctx);
    const meta = await ctx.db.system.get(storageId);
    if (!meta) throw new Error("Upload not found");
    if (meta.size > 32 * 1024 * 1024) {
      await ctx.storage.delete(storageId);
      throw new Error("File too big");
    }
    for (const old of await ctx.db.query("gameFiles").withIndex("by_name", (q) => q.eq("name", name)).collect()) {
      await ctx.storage.delete(old.storageId);
      await ctx.db.delete(old._id);
    }
    await ctx.db.insert("gameFiles", { name, storageId, size: meta.size, uploadedBy: user._id });
  },
});
