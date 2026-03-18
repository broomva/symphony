import "server-only";
import { eq } from "drizzle-orm";
import { db } from "./client";
import {
  dashboardSettings,
  type DashboardSettings,
  type User,
  user,
} from "./schema";

export async function getUserById({
  userId,
}: {
  userId: string;
}): Promise<User | undefined> {
  const users = await db
    .select()
    .from(user)
    .where(eq(user.id, userId))
    .limit(1);
  return users[0];
}

export async function getDashboardSettings({
  userId,
}: {
  userId: string;
}): Promise<DashboardSettings | undefined> {
  const [settings] = await db
    .select()
    .from(dashboardSettings)
    .where(eq(dashboardSettings.userId, userId))
    .limit(1);
  return settings;
}

export async function upsertDashboardSettings({
  userId,
  symphonyUrl,
  theme,
  refreshIntervalMs,
}: {
  userId: string;
  symphonyUrl?: string;
  theme?: string;
  refreshIntervalMs?: number;
}): Promise<void> {
  await db
    .insert(dashboardSettings)
    .values({
      userId,
      ...(symphonyUrl !== undefined && { symphonyUrl }),
      ...(theme !== undefined && { theme }),
      ...(refreshIntervalMs !== undefined && { refreshIntervalMs }),
    })
    .onConflictDoUpdate({
      target: dashboardSettings.userId,
      set: {
        ...(symphonyUrl !== undefined && { symphonyUrl }),
        ...(theme !== undefined && { theme }),
        ...(refreshIntervalMs !== undefined && { refreshIntervalMs }),
        updatedAt: new Date(),
      },
    });
}
