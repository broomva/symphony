export const dashboardConfig = {
  appName: "Symphony",
  appTitle: "Symphony Dashboard",
  appDescription: "Orchestration dashboard for Symphony coding agents",
  appUrl: process.env.APP_URL ?? "http://localhost:3000",
  defaultRefreshInterval: 5000,
  navigation: [
    { title: "Overview", href: "/overview", icon: "LayoutDashboard" },
    { title: "Issues", href: "/issues", icon: "ListTodo" },
    { title: "Workspaces", href: "/workspaces", icon: "FolderOpen" },
    { title: "Metrics", href: "/metrics", icon: "BarChart3" },
    { title: "Controls", href: "/controls", icon: "Settings" },
  ],
} as const;
