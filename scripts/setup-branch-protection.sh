#!/bin/sh
# Setup GitHub branch protection rules for master
# Run once: bash scripts/setup-branch-protection.sh
#
# Requires: gh CLI authenticated with admin access
set -e

REPO="broomva/symphony"
BRANCH="master"

echo "Setting up branch protection for ${REPO}:${BRANCH}..."

gh api -X PUT "repos/${REPO}/branches/${BRANCH}/protection" \
  --input - <<'EOF'
{
  "required_status_checks": {
    "strict": true,
    "contexts": [
      "Check & Lint",
      "Test",
      "Control Audit"
    ]
  },
  "enforce_admins": false,
  "required_pull_request_reviews": null,
  "restrictions": null,
  "allow_auto_merge": true,
  "required_linear_history": true,
  "allow_force_pushes": false,
  "allow_deletions": false
}
EOF

# Enable auto-merge on the repo
gh api -X PATCH "repos/${REPO}" \
  --field allow_auto_merge=true \
  --field allow_squash_merge=true \
  --field squash_merge_commit_title=PR_TITLE \
  --field squash_merge_commit_message=PR_BODY \
  --field delete_branch_on_merge=true

echo ""
echo "Branch protection configured:"
echo "  Required checks: Check & Lint, Test, Control Audit"
echo "  Auto-merge: enabled (squash)"
echo "  Linear history: required"
echo "  Delete branch on merge: enabled"
echo ""
echo "PRs will auto-merge when all control metalayer gates pass."
