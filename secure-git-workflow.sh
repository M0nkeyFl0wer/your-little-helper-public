#!/bin/bash
# Secure Git Workflow for Little Helper
# 
# This script provides secure ways to work on seshat without storing
# GitHub credentials on the shared server.
#
# Options:
# 1. SSH Agent Forwarding (keys stay on laptop)
# 2. Sync changes back to laptop for pushing
# 3. Use HTTPS with limited-scope token (if needed)

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
SESHAT_HOST="seshat.noosworx.com"
SESHAT_USER="m0nkey-fl0wer"
SESHAT_PORT="8888"
LOCAL_PROJECT="$HOME/Projects/little-helper"
SESHAT_PROJECT="/home/$SESHAT_USER/Projects/little-helper"

show_help() {
    echo "Secure Git Workflow for Little Helper"
    echo ""
    echo "Usage: $0 [command]"
    echo ""
    echo "Commands:"
    echo "  forward         Enable SSH agent forwarding to seshat (secure)"
    echo "  sync-to-seshat  Push local changes to seshat for compilation"
    echo "  sync-from-seshat  Pull changes from seshat to local (then push from here)"
    echo "  build-on-seshat  Run cargo build on seshat (uses agent forwarding)"
    echo "  setup-token     Setup GitHub token for HTTPS auth (limited scope)"
    echo "  check-security  Verify no keys on seshat"
    echo "  help            Show this help message"
    echo ""
    echo "Recommended workflow:"
    echo "  1. Work on seshat via 'forward' or 'sync-to-seshat'"
    echo "  2. When ready to commit: 'sync-from-seshat'"
    echo "  3. Push from local machine (where GH keys are safe)"
}

check_security() {
    echo -e "${YELLOW}Checking seshat for GitHub credentials...${NC}"
    
    # Check for SSH keys
    KEYS=$(ssh -p $SESHAT_PORT $SESHAT_USER@$SESHAT_HOST "ls ~/.ssh/id_* 2>/dev/null | wc -l")
    
    if [ "$KEYS" -gt 0 ]; then
        echo -e "${RED}⚠️  WARNING: Found $KEYS SSH key files on seshat${NC}"
        ssh -p $SESHAT_PORT $SESHAT_USER@$SESHAT_HOST "ls -la ~/.ssh/id_*"
        echo ""
        echo -e "${YELLOW}To remove them:${NC}"
        echo "  ssh $SESHAT_USER@$SESHAT_HOST -p $SESHAT_PORT 'rm ~/.ssh/id_*'"
    else
        echo -e "${GREEN}✓ No SSH keys found on seshat${NC}"
    fi
    
    # Check for git credentials
    CREDS=$(ssh -p $SESHAT_PORT $SESHAT_USER@$SESHAT_HOST "ls ~/.git-credentials 2>/dev/null | wc -l")
    if [ "$CREDS" -gt 0 ]; then
        echo -e "${YELLOW}ℹ️  Git credentials file exists on seshat${NC}"
    fi
}

setup_agent_forwarding() {
    echo -e "${GREEN}Setting up SSH agent forwarding...${NC}"
    echo ""
    echo -e "${YELLOW}Step 1:${NC} Ensure your SSH key is loaded in the agent:"
    echo "  ssh-add -l"
    echo ""
    echo -e "${YELLOW}Step 2:${NC} Connect to seshat with agent forwarding:"
    echo "  ssh -A -p $SESHAT_PORT $SESHAT_USER@$SESHAT_HOST"
    echo ""
    echo -e "${YELLOW}Step 3:${NC} Once on seshat, test GitHub access:"
    echo "  ssh -T git@github.com"
    echo ""
    echo -e "${GREEN}Your keys stay on your laptop - seshat just forwards the authentication!${NC}"
    echo ""
    read -p "Press Enter to connect now, or Ctrl+C to cancel..."
    ssh -A -p $SESHAT_PORT $SESHAT_USER@$SESHAT_HOST
}

sync_to_seshat() {
    echo -e "${GREEN}Syncing local changes to seshat...${NC}"
    
    # Create project dir on seshat if needed
    ssh -p $SESHAT_PORT $SESHAT_USER@$SESHAT_HOST "mkdir -p $SESHAT_PROJECT"
    
    # Sync files (exclude target/, .git/, large files)
    rsync -avz --progress \
        --exclude 'target/' \
        --exclude '.git/' \
        --exclude '*.rs.bk' \
        --exclude 'node_modules/' \
        -e "ssh -p $SESHAT_PORT" \
        "$LOCAL_PROJECT/" \
        "$SESHAT_USER@$SESHAT_HOST:$SESHAT_PROJECT/"
    
    echo ""
    echo -e "${GREEN}✓ Synced to seshat${NC}"
    echo -e "${YELLOW}Next:${NC} SSH to seshat and work there:"
    echo "  ssh -p $SESHAT_PORT $SESHAT_USER@$SESHAT_HOST"
    echo "  cd $SESHAT_PROJECT"
}

sync_from_seshat() {
    echo -e "${GREEN}Syncing changes from seshat to local...${NC}"
    
    # Pull changes back (but protect local git and sensitive files)
    rsync -avz --progress \
        --exclude '.git/' \
        --exclude 'target/' \
        -e "ssh -p $SESHAT_PORT" \
        "$SESHAT_USER@$SESHAT_HOST:$SESHAT_PROJECT/" \
        "$LOCAL_PROJECT/"
    
    echo ""
    echo -e "${GREEN}✓ Synced from seshat${NC}"
    echo -e "${YELLOW}Next:${NC} Review changes locally and push from your machine:"
    echo "  cd $LOCAL_PROJECT"
    echo "  git status"
    echo "  git diff"
    echo "  git add ."
    echo "  git commit -m 'your message'"
    echo "  git push"
}

build_on_seshat() {
    echo -e "${GREEN}Building on seshat with agent forwarding...${NC}"
    ssh -A -p $SESHAT_PORT $SESHAT_USER@$SESHAT_HOST "cd $SESHAT_PROJECT && cargo build -p agent_host"
}

setup_https_token() {
    echo -e "${YELLOW}Setting up GitHub token for HTTPS auth...${NC}"
    echo ""
    echo -e "${RED}⚠️  Only use this if agent forwarding doesn't work for you${NC}"
    echo ""
    echo "Steps:"
    echo "1. Create a token at: https://github.com/settings/tokens"
    echo "2. Give it ONLY 'repo' scope (minimum needed)"
    echo "3. On seshat, run:"
    echo ""
    echo "   git remote set-url origin https://<token>@github.com/M0nkeyFl0wer/your-little-helper.git"
    echo ""
    echo "4. Test with: git fetch"
    echo ""
    echo -e "${YELLOW}Security notes:${NC}"
    echo "- Token will be stored in plain text in .git/config on seshat"
    echo "- Anyone with seshat root access can see it"
    echo "- Revoke the token immediately when done: https://github.com/settings/tokens"
}

case "${1:-help}" in
    forward)
        setup_agent_forwarding
        ;;
    sync-to-seshat)
        sync_to_seshat
        ;;
    sync-from-seshat)
        sync_from_seshat
        ;;
    build-on-seshat)
        build_on_seshat
        ;;
    setup-token)
        setup_https_token
        ;;
    check-security)
        check_security
        ;;
    help|--help|-h)
        show_help
        ;;
    *)
        echo -e "${RED}Unknown command: $1${NC}"
        show_help
        exit 1
        ;;
esac
