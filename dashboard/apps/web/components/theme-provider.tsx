"use client";

import { ThemeProvider as NextThemesProvider } from "next-themes";

type NextProviderProps = Parameters<typeof NextThemesProvider>[0];

export function ThemeProvider({ children, ...props }: NextProviderProps) {
  return <NextThemesProvider {...props}>{children}</NextThemesProvider>;
}
