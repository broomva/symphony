import { nextCookies } from "better-auth/next-js";
import { createAuthClient } from "better-auth/react";

// Better Auth auto-detects the base URL from window.location.origin on client
// and uses relative URLs for SSR, so we don't need to specify baseURL
const authClient = createAuthClient({
  plugins: [nextCookies()],
});

export default authClient;
