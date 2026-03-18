import { Badge } from "@/components/ui/badge";

type Status = "running" | "retrying" | "offline" | "online" | "idle";

const statusColors: Record<Status, string> = {
  running: "bg-green-500/10 text-green-500 border-green-500/20",
  retrying: "bg-yellow-500/10 text-yellow-500 border-yellow-500/20",
  offline: "bg-red-500/10 text-red-500 border-red-500/20",
  online: "bg-green-500/10 text-green-500 border-green-500/20",
  idle: "bg-gray-500/10 text-gray-500 border-gray-500/20",
};

interface StatusBadgeProps {
  status: Status;
  label?: string;
}

export function StatusBadge({ status, label }: StatusBadgeProps) {
  return (
    <Badge variant="outline" className={statusColors[status]}>
      {label ?? status}
    </Badge>
  );
}
