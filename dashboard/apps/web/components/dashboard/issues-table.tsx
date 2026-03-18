"use client";

import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { StatusBadge } from "./status-badge";
import type { RunningInfo, RetryingInfo } from "@symphony/client";
import Link from "next/link";

interface IssuesTableProps {
  running: RunningInfo[];
  retrying: RetryingInfo[];
}

export function IssuesTable({ running, retrying }: IssuesTableProps) {
  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-lg font-semibold mb-2">
          Running ({running.length})
        </h3>
        <div className="rounded-md border">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Identifier</TableHead>
                <TableHead>State</TableHead>
                <TableHead>Session</TableHead>
                <TableHead>Turns</TableHead>
                <TableHead className="text-right">Tokens</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {running.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={5} className="text-center text-muted-foreground">
                    No running issues
                  </TableCell>
                </TableRow>
              ) : (
                running.map((issue) => (
                  <TableRow key={issue.issue_id}>
                    <TableCell>
                      <Link
                        href={`/issues/${encodeURIComponent(issue.identifier)}`}
                        className="font-medium hover:underline"
                      >
                        {issue.identifier}
                      </Link>
                    </TableCell>
                    <TableCell>
                      <StatusBadge status="running" label={issue.state} />
                    </TableCell>
                    <TableCell className="text-muted-foreground text-sm">
                      {issue.session_id ?? "\u2014"}
                    </TableCell>
                    <TableCell>{issue.turn_count}</TableCell>
                    <TableCell className="text-right tabular-nums">
                      {issue.tokens.total_tokens.toLocaleString()}
                    </TableCell>
                  </TableRow>
                ))
              )}
            </TableBody>
          </Table>
        </div>
      </div>

      <div>
        <h3 className="text-lg font-semibold mb-2">
          Retrying ({retrying.length})
        </h3>
        <div className="rounded-md border">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Identifier</TableHead>
                <TableHead>Attempt</TableHead>
                <TableHead>Due At</TableHead>
                <TableHead>Error</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {retrying.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={4} className="text-center text-muted-foreground">
                    No retrying issues
                  </TableCell>
                </TableRow>
              ) : (
                retrying.map((issue) => (
                  <TableRow key={issue.issue_id}>
                    <TableCell>
                      <Link
                        href={`/issues/${encodeURIComponent(issue.identifier)}`}
                        className="font-medium hover:underline"
                      >
                        {issue.identifier}
                      </Link>
                    </TableCell>
                    <TableCell>{issue.attempt}</TableCell>
                    <TableCell className="tabular-nums">
                      {new Date(issue.due_at_ms).toLocaleString()}
                    </TableCell>
                    <TableCell className="text-muted-foreground text-sm max-w-xs truncate">
                      {issue.error ?? "\u2014"}
                    </TableCell>
                  </TableRow>
                ))
              )}
            </TableBody>
          </Table>
        </div>
      </div>
    </div>
  );
}
