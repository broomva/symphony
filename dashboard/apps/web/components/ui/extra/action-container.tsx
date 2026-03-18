"use client";

import Link from "next/link";
import type * as React from "react";

import { cn } from "@/lib/utils";

type ActionContainerProps = React.ComponentProps<"div">;

function ActionContainer({ className, ...props }: ActionContainerProps) {
  return (
    <div
      className={cn(
        "group relative rounded-xl border border-border/60 bg-muted/20 px-4 py-3 transition-all duration-200 hover:border-primary/25",
        className
      )}
      {...props}
    />
  );
}

type ActionContainerLinkProps = React.ComponentProps<typeof Link>;

function ActionContainerLink({
  className,
  tabIndex,
  ...props
}: ActionContainerLinkProps) {
  return (
    <Link
      className={cn("absolute inset-0 z-10", className)}
      tabIndex={tabIndex ?? -1}
      {...props}
    />
  );
}

type ActionContainerTopProps = React.ComponentProps<"div">;

function ActionContainerTop({ className, ...props }: ActionContainerTopProps) {
  return <div className={cn("z-20", className)} {...props} />;
}

export { ActionContainer, ActionContainerLink, ActionContainerTop };
