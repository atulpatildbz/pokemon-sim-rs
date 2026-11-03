import { defineSchema, defineTable } from "convex/server";
import { authTables } from "@convex-dev/auth/server";
import { v } from "convex/values";

export default defineSchema({
  ...authTables,
  // Who may sign in (checked before a session is created, and again on every call).
  allowedEmails: defineTable({
    email: v.string(),
    createdAt: v.number(),
  }).index("by_email", ["email"]),
  // The web build's synced files (save slots), one row per user and path. `data: null` marks a
  // deleted file, so the deletion reaches other devices. `modified` is the file's own time
  // (the device's clock): the newest version wins.
  files: defineTable({
    userId: v.id("users"),
    path: v.string(),
    data: v.union(v.bytes(), v.null()),
    modified: v.number(),
    // Random id of the browser that wrote it (to tell other devices' changes from our own).
    device: v.string(),
    updatedAt: v.number(),
  }).index("by_user_path", ["userId", "path"]),
  // The player's own copy of PokeGame.exe / POKéGAME.dat, so a new device needs no files.
  // Shared by the allowed accounts; never public.
  gameFiles: defineTable({
    name: v.union(v.literal("exe"), v.literal("dat")),
    storageId: v.id("_storage"),
    size: v.number(),
    uploadedBy: v.id("users"),
  }).index("by_name", ["name"]),
});
