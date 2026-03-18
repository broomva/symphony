"use client";

import {
  Activity,
  BarChart3,
  FolderOpen,
  LayoutDashboard,
  ListTodo,
  Settings,
} from "lucide-react";
import Link from "next/link";
import { usePathname } from "next/navigation";

import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarFooter,
} from "@/components/ui/sidebar";
import { ConnectionIndicator } from "@/components/dashboard/connection-indicator";

const navItems = [
  { title: "Overview", href: "/overview" as const, icon: LayoutDashboard },
  { title: "Issues", href: "/issues" as const, icon: ListTodo },
  { title: "Workspaces", href: "/workspaces" as const, icon: FolderOpen },
  { title: "Metrics", href: "/metrics" as const, icon: BarChart3 },
  { title: "Controls", href: "/controls" as const, icon: Settings },
] as const;

export function AppSidebar() {
  const pathname = usePathname();

  return (
    <Sidebar>
      <SidebarHeader className="border-b px-4 py-3">
        <Link href="/overview" className="flex items-center gap-2">
          <Activity className="h-6 w-6" />
          <span className="text-lg font-bold">Symphony</span>
        </Link>
      </SidebarHeader>
      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupLabel>Navigation</SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu>
              {navItems.map((item) => (
                <SidebarMenuItem key={item.href}>
                  <SidebarMenuButton
                    asChild
                    isActive={pathname.startsWith(item.href)}
                  >
                    <Link href={item.href}>
                      <item.icon className="h-4 w-4" />
                      <span>{item.title}</span>
                    </Link>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              ))}
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      </SidebarContent>
      <SidebarFooter className="border-t px-4 py-3">
        <ConnectionIndicator />
      </SidebarFooter>
    </Sidebar>
  );
}
