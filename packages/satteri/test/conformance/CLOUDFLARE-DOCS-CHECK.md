# Cloudflare docs conformance check

- Root: `/home/erika/Projects/cloudflare-docs`
- Features: MDX + frontmatter + directive
- Files: 8512
- MDAST: 7627 ok / 885 fail
- HAST:  7607 ok / 905 fail
- Parse errors: 0


## MDAST mismatches — 37 unique pattern(s)

### 512× `$.children[N].attributes[N].value.value`
- src/content/docs/ai-gateway/demos.mdx
  - $.children[5].attributes[0].value.value: "[\n  \t\"reference-architecture\",\n  \t\"design-guide\",\n  \t\"reference-arch vs "[\n\t\t\"reference-architecture\",\n\t\t\"design-guide\",\n\t\t\"reference-arch
- src/content/docs/ai-gateway/evaluations/add-human-feedback-api.mdx
  - $.children[27].attributes[2].value.value: "{\n  \tfeedback: 1,\n  }" vs "{\n\t\tfeedback: 1,\n\t}"
- src/content/docs/ai-gateway/integrations/aig-workers-ai-binding.mdx
  - $.children[10].attributes[2].value.value: "{\n  \tcategory: \"hello-world\",\n  \ttype: \"Worker only\",\n  \tlang: \"Type vs "{\n\t\tcategory: \"hello-world\",\n\t\ttype: \"Worker only\",\n\t\tlang: \"Type
  … and 509 more

### 120× `$.children[N].children[N].children[N].attributes[N].value.value`
- src/content/docs/agents/guides/remote-mcp-server.mdx
  - $.children[17].children[0].children[1].attributes[2].value.value: "\n   \"remote-mcp-server-authless --template=cloudflare/ai/demos/remote-mcp-aut vs "\n\t\t\"remote-mcp-server-authless --template=cloudflare/ai/demos/remote-mcp-au
- src/content/docs/byoip/address-maps/setup.mdx
  - $.children[6].children[1].children[0].attributes[2].value.value: "{\n  description: \"Example address map\",\n  enabled: true,\n  ips: [\n    \"2 vs "{\n    description: \"Example address map\",\n    enabled: true,\n    ips: [\n 
- src/content/docs/byoip/service-bindings/magic-transit-with-cdn.mdx
  - $.children[6].children[1].children[1].attributes[2].value.value: "{\n\tpre_existing_product: \"Magic Transit\",\n\tadded_product: \"CDN\",\n}" vs "{\n\t\tpre_existing_product: \"Magic Transit\",\n\t\tadded_product: \"CDN\",\n\
  … and 117 more

### 51× `$.children[N].children[N].children[N].children[N].referenceType`
- src/content/changelog/workflows/2026-04-15-workflows-limits-raised.mdx
  - $.children[2].children[3].children[0].children[1].referenceType: missing in expected
- src/content/docs/agents/platform/limits.mdx
  - $.children[4].children[1].children[1].children[1].referenceType: missing in expected
- src/content/docs/browser-run/limits.mdx
  - $.children[6].children[2].children[0].children[1].referenceType: missing in expected
  … and 48 more

### 34× `$.children[N].children[N].referenceType`
- src/content/docs/analytics/graphql-api/tutorials/end-customer-analytics.mdx
  - $.children[3].children[6].referenceType: missing in expected
- src/content/docs/analytics/graphql-api/tutorials/querying-access-login-events.mdx
  - $.children[3].children[4].referenceType: missing in expected
- src/content/docs/analytics/graphql-api/tutorials/querying-email-routing.mdx
  - $.children[3].children[4].referenceType: missing in expected
  … and 31 more

### 24× `$.children`
- src/content/docs/ai-gateway/usage/providers/anthropic.mdx
  - $.children: array length 14 vs 16
- src/content/docs/ai-gateway/usage/providers/cerebras.mdx
  - $.children: array length 12 vs 14
- src/content/docs/ai-gateway/usage/providers/cohere.mdx
  - $.children: array length 17 vs 19
  … and 21 more

### 16× `$.children[N].children[N].children`
- src/content/docs/ai-search/how-to/nlweb.mdx
  - $.children[4].children[0].children: array length 3 vs 1
- src/content/docs/browser-run/features/webmcp.mdx
  - $.children[37].children[2].children: array length 4 vs 2
- src/content/docs/cloudflare-one/networks/connectors/cloudflare-tunnel/private-net/cloudflared/private-dns.mdx
  - $.children[5].children[1].children: array length 2 vs 4
  … and 13 more

### 15× `$.children[N].children[N].attributes[N].value.value`
- src/content/docs/cache/how-to/cache-response-rules/create-api.mdx
  - $.children[10].children[0].attributes[3].value.value: "{\n  rules: [\n    {\n      expression: 'http.request.uri.path.extension eq \"j vs "{\n    rules: [\n      {\n        expression: 'http.request.uri.path.extension 
- src/content/docs/cache/how-to/cache-rules/create-api.mdx
  - $.children[10].children[0].attributes[2].value.value: "{\n  rules: [\n    {\n      expression: '(http.host eq \"example.com\")',\n     vs "{\n    rules: [\n      {\n        expression: '(http.host eq \"example.com\")',
- src/content/docs/data-localization/metadata-boundary/get-started.mdx
  - $.children[12].children[1].attributes[2].value.value: "{\n  regions: \"eu\",\n  allow_out_of_region_access: false\n}" vs "{\n    regions: \"eu\",\n    allow_out_of_region_access: false\n  }"
  … and 12 more

### 14× `$.children[N].children[N].children[N].children`
- src/content/changelog/browser-run/2025-07-28-br-pricing.mdx
  - $.children[5].children[2].children[3].children: array length 7 vs 6
- src/content/changelog/waf/2025-08-11-waf-release.mdx
  - $.children[8].children[1].children[9].children: array length 3 vs 7
- src/content/changelog/waf/2025-12-18-waf-release.mdx
  - $.children[5].children[1].children[1].children: array length 3 vs 7
  … and 11 more

### 13× `$.children[N].children[N].children[N].children[N].attributes[N].value.value`
- src/content/docs/artifacts/get-started/workers.mdx
  - $.children[9].children[0].children[0].children[2].attributes[2].value.value: "{\n   category: \"hello-world\",\n   type: \"Worker only\",\n   lang: \"TypeScr vs "{\n\t\tcategory: \"hello-world\",\n\t\ttype: \"Worker only\",\n\t\tlang: \"Type
- src/content/docs/cloudflare-for-platforms/cloudflare-for-saas/security/certificate-management/enforce-mtls.mdx
  - $.children[18].children[0].children[1].children[5].attributes[2].value.value: "{\n  \tssl: {\n  \t\tmethod: \"http\",\n  \t\ttype: \"dv\",\n  \t\tsettings: {\ vs "{\n\t\tssl: {\n\t\t\tmethod: \"http\",\n\t\t\ttype: \"dv\",\n\t\t\tsettings: {\
- src/content/docs/d1/tutorials/build-a-comments-api.mdx
  - $.children[6].children[0].children[0].children[2].attributes[2].value.value: "{\n   category: \"hello-world\",\n   type: \"Worker only\",\n   lang: \"TypeScr vs "{\n\t\tcategory: \"hello-world\",\n\t\ttype: \"Worker only\",\n\t\tlang: \"Type
  … and 10 more

### 13× `$.children[N].children[N].children[N].children[N].children[N].attributes[N].value.value`
- src/content/docs/cloudflare-one/access-controls/access-settings/independent-mfa.mdx
  - $.children[7].children[1].children[0].children[1].children[1].attributes[2].value.value: "{\n   \tauth_domain: \"your-team-name.cloudflareaccess.com\",\n   \tname: \"You vs "{\n\t\tauth_domain: \"your-team-name.cloudflareaccess.com\",\n\t\tname: \"Your 
- src/content/docs/cloudflare-one/access-controls/ai-controls/linked-apps.mdx
  - $.children[11].children[1].children[0].children[1].children[1].attributes[2].value.value: "{\n   name: \"Allow MCP server\",\n   decision: \"non_identity\",\n   include:  vs "{\n\t\tname: \"Allow MCP server\",\n\t\tdecision: \"non_identity\",\n\t\tinclud
- src/content/docs/cloudflare-one/access-controls/ai-controls/mcp-portals.mdx
  - $.children[46].children[1].children[0].children[1].children[1].attributes[2].value.value: "{\n   allow_code_mode: false,\n}" vs "{\n\t\tallow_code_mode: false,\n\t}"
  … and 10 more

### 12× `$.children[N].children`
- src/content/docs/ai-gateway/configuration/custom-costs.mdx
  - $.children[9].children: array length 3 vs 5
- src/content/docs/analytics/analytics-engine/sql-reference/string-functions.mdx
  - $.children[69].children: array length 7 vs 5
- src/content/docs/analytics/graphql-api/getting-started/querying-basics.mdx
  - $.children[14].children: array length 3 vs 5
  … and 9 more

### 10× `$.children[N].children[N].children[N].type`
- src/content/docs/cloudflare-one/access-controls/applications/http-apps/saas-apps/atlassian-saas.mdx
  - $.children[9].children[3].children[1].type: "containerDirective" vs "paragraph"
- src/content/docs/cloudflare-one/access-controls/policies/index.mdx
  - $.children[47].children[0].children[1].type: "table" vs "paragraph"
- src/content/docs/cloudflare-one/access-controls/policies/mfa-requirements.mdx
  - $.children[9].children[4].children[1].type: "table" vs "paragraph"
  … and 7 more

### 5× `$.children[N].type`
- src/content/docs/cloudflare-one/email-security/setup/index.mdx
  - $.children[5].type: "table" vs "paragraph"
- src/content/docs/cloudflare-one/insights/analytics/shadow-it-discovery.mdx
  - $.children[14].type: "table" vs "paragraph"
- src/content/partials/networking-services/mnm/rules/static-threshold.mdx
  - $.children[18].type: "table" vs "paragraph"
  … and 2 more

### 5× `$.children[N].children[N].spread`
- src/content/docs/ddos-protection/managed-rulesets/http/http-overrides/configure-api.mdx
  - $.children[9].children[1].spread: false vs true
- src/content/docs/ddos-protection/managed-rulesets/network/network-overrides/configure-api.mdx
  - $.children[8].children[1].spread: false vs true
- src/content/docs/ssl/edge-certificates/additional-options/certificate-transparency-monitoring.mdx
  - $.children[17].children[0].spread: true vs false
  … and 2 more

### 4× `$.children[N].children[N].children[N].children[N].value`
- src/content/partials/waf/managed-rules-browse-zone-new-nav.mdx
  - $.children[2].children[0].children[3].children[1].value: "props.rulesetName == \"Cloudflare Managed Ruleset\" && (\n   <Image src={cloudf vs "props.rulesetName == \"Cloudflare Managed Ruleset\" && (\n  <Image src={cloudfl
- src/content/partials/waf/managed-rules-browse-zone.mdx
  - $.children[3].children[0].children[3].children[1].value: "(props.rulesetName == \"\" || props.rulesetName == \"Cloudflare Managed Ruleset vs "(props.rulesetName == \"\" || props.rulesetName == \"Cloudflare Managed Ruleset
- src/content/partials/waf/managed-ruleset-configure-individual-rules.mdx
  - $.children[3].children[0].children[1].children[2].value: "(props.rulesetName == \"\" || props.rulesetName == \"Cloudflare Managed Ruleset vs "(props.rulesetName == \"\" || props.rulesetName == \"Cloudflare Managed Ruleset
  … and 1 more

### 3× `$.children[N].children[N].children[N].spread`
- src/content/docs/api-shield/security/jwt-validation/jwt-worker.mdx
  - $.children[13].children[0].children[4].spread: true vs false
- src/content/docs/cloudflare-one/traffic-policies/get-started/dns.mdx
  - $.children[18].children[0].children[0].spread: false vs true
- src/content/docs/learning-paths/secure-internet-traffic/build-dns-policies/create-policy.mdx
  - $.children[5].children[0].children[0].spread: false vs true

### 3× `$.children[N].children[N].children[N].children[N].children[N].children[N].referenceType`
- src/content/docs/bots/plans/biz-and-ent.mdx
  - $.children[4].children[1].children[1].children[1].children[0].children[1].referenceType: missing in expected
- src/content/docs/dns/reference/domain-connect.mdx
  - $.children[11].children[1].children[1].children[2].children[0].children[3].referenceType: missing in expected
- src/content/partials/cloudflare-one/tunnel/troubleshoot-private-networks.mdx
  - $.children[19].children[2].children[1].children[0].children[0].children[9].referenceType: missing in expected

### 3× `$.children[N].children[N].children[N].children[N].children[N].children[N].attributes[N].value.value`
- src/content/docs/cloudflare-one/networks/resolvers-and-proxies/proxy-endpoints/index.mdx
  - $.children[27].children[1].children[0].children[1].children[0].children[1].attributes[2].value.value: "{\n   kind: \"identity\",\n   name: \"any_name\",\n}" vs "{\n\t\tkind: \"identity\",\n\t\tname: \"any_name\",\n\t}"
- src/content/docs/d1/get-started.mdx
  - $.children[15].children[0].children[0].children[0].children[0].children[2].attributes[2].value.value: "{\n   category: \"hello-world\",\n   type: \"Worker only\",\n   lang: \"TypeScr vs "{\n \tcategory: \"hello-world\",\n \ttype: \"Worker only\",\n \tlang: \"TypeScr
- src/content/docs/kv/get-started.mdx
  - $.children[15].children[0].children[1].children[0].children[0].children[2].attributes[2].value.value: "{\n   category: \"hello-world\",\n   type: \"Worker only\",\n   lang: \"TypeScr vs "{\n \tcategory: \"hello-world\",\n \ttype: \"Worker only\",\n \tlang: \"TypeScr

### 3× `$.children[N].children[N].children[N].referenceType`
- src/content/docs/cloudflare-one/team-and-resources/devices/cloudflare-one-client/deployment/mdm-deployment/parameters.mdx
  - $.children[88].children[0].children[1].referenceType: missing in expected
- src/content/docs/cloudflare-one/team-and-resources/devices/cloudflare-one-client/deployment/mdm-deployment/switch-organizations.mdx
  - $.children[18].children[0].children[3].referenceType: missing in expected
- src/content/partials/dns/conversion-subdomain-setup-callout.mdx
  - $.children[1].children[1].children[1].referenceType: missing in expected

### 3× `$.children[N].children[N].children[N].alt`
- src/content/docs/style-guide/components/index.mdx
  - $.children[8].children[0].children[0].alt: "DashButton component\nexample" vs "DashButton component example"
- src/content/partials/networking-services/mnm-magic-transit-integration.mdx
  - $.children[9].children[0].children[0].alt: "You can create rules to activate Magic Transit automatically, to protect your I vs "You can create rules to activate Magic Transit automatically, to protect your I
- src/content/partials/style-guide/llms-txt.mdx
  - $.children[8].children[0].children[0].alt: "Page options\nbutton" vs "Page options button"

### 2× `$.children[N].children[N].children[N].children[N].type`
- src/content/docs/analytics/graphql-api/getting-started/authentication/index.mdx
  - $.children[4].children[1].children[1].children[1].type: "mdxJsxFlowElement" vs "paragraph"
- src/content/docs/email-security/migrate-to-email-security.mdx
  - $.children[7].children[0].children[0].children[0].type: "link" vs "text"

### 2× `$.children[N].children[N].children[N].children[N].children[N].spread`
- src/content/docs/api-shield/management-and-monitoring/developer-portal.mdx
  - $.children[4].children[0].children[0].children[0].children[3].spread: true vs false
- src/content/partials/api-shield/set-up-session-identifiers.mdx
  - $.children[2].children[0].children[0].children[0].children[4].spread: true vs false

### 2× `$.children[N].children[N].children[N].children[N].children[N].referenceType`
- src/content/docs/cloudflare-one/team-and-resources/devices/cloudflare-one-client/configure/settings/index.mdx
  - $.children[98].children[1].children[4].children[2].children[1].referenceType: missing in expected
- src/content/docs/dns/manage-dns-records/reference/wildcard-dns-records.mdx
  - $.children[18].children[3].children[1].children[0].children[1].referenceType: missing in expected

### 2× `$.children[N].children[N].children[N].value`
- src/content/partials/cloudflare-one/access/self-hosted-app/create-app.mdx
  - $.children[2].children[4].children[2].value: "\nprops.private && (\n   <p> \t\t\tIf the application is non-HTTPS or you do no vs "\nprops.private && (\n  <p> \t\t\tIf the application is non-HTTPS or you do not
- src/content/partials/networking-services/routing/bgp-config-steps.mdx
  - $.children[2].children[1].children[1].value: " props.magicWord !== \"Magic Transit\" && (\n   <>\n   <Aside type=\"note\">Mul vs " props.magicWord !== \"Magic Transit\" && (\n  <>\n  <Aside type=\"note\">Multi

### 2× `$.children[N].children[N].children[N].children[N].children[N].children[N].value`
- src/content/partials/waf/dash-configure-all-rules.mdx
  - $.children[2].children[0].children[0].children[0].children[5].children[2].value: "props.rulesetName == \"Cloudflare Managed Ruleset\" && (\n   <Image src={cloudf vs "props.rulesetName == \"Cloudflare Managed Ruleset\" && (\n  <Image src={cloudfl
- src/content/partials/waf/dash-deploy-managed-ruleset-zone.mdx
  - $.children[2].children[0].children[0].children[0].children[2].children[0].value: "props.dashOptionName ? (\n   <Markdown text={`Turn on <strong>${props.dashOptio vs "props.dashOptionName ? (\n  <Markdown text={`Turn on <strong>${props.dashOption

### 1× `$.children[N].children[N].value`
- src/content/changelog/containers/2025-11-21-new-cpu-pricing.mdx
  - $.children[7].children[0].value: "CPU cost would have been: " vs "CPU cost would have been: **"

### 1× `$.children[N].children[N].attributes[N].value`
- src/content/docs/billing/index.mdx
  - $.children[14].children[4].attributes[0].value: "Resolve \"you cannot modify this subscription\"" vs "Resolve &quot;you cannot modify this subscription&quot;"

### 1× `$.children[N].children[N].children[N].children[N].children[N].children[N].children[N].attributes[N].value.value`
- src/content/docs/cloudflare-one/networks/connectors/cloudflare-tunnel/configure-tunnels/remote-tunnel-permissions.mdx
  - $.children[9].children[0].children[1].children[1].children[0].children[1].children[1].attributes[2].value.value: "{\n  \t\tname: \"Example tunnel\",\n  \t\ttunnel_secret: \"AQIDBAUGBwgBAgMEBQYH vs "{\n\t\t\tname: \"Example tunnel\",\n\t\t\ttunnel_secret: \"AQIDBAUGBwgBAgMEBQYH

### 1× `$.children[N].children[N].children[N].children[N].children[N].children[N].children[N].type`
- src/content/docs/cloudflare-one/networks/connectors/cloudflare-tunnel/private-net/cloudflared/tunnel-virtual-networks.mdx
  - $.children[11].children[0].children[1].children[0].children[1].children[0].children[1].type: "containerDirective" vs "paragraph"

### 1× `$.children[N].children[N].children[N].children[N].spread`
- src/content/docs/load-balancing/additional-options/spectrum.mdx
  - $.children[8].children[5].children[1].children[0].spread: true vs false

### 1× `$.children[N].children[N].children[N].children[N].children[N].children[N].spread`
- src/content/docs/ssl/edge-certificates/custom-certificates/uploading.mdx
  - $.children[9].children[0].children[1].children[5].children[1].children[1].spread: true vs false

### 1× `$.children[N].children[N].alt`
- src/content/docs/support/third-party-software/content-management-system-cms/how-do-i-enable-http2-server-push-in-wordpress.mdx
  - $.children[2].children[0].alt: "Old URL: https://support.cloudflare.com/hc/en-us/article_attachments/1150057333 vs "Old URL: https://support.cloudflare.com/hc/en-us/article_attachments/1150057333

### 1× `$.children[N].children[N].children[N].children[N].children[N].attributes[N].value`
- src/content/docs/workers/runtime-apis/html-rewriter.mdx
  - $.children[29].children[12].children[0].children[0].children[1].attributes[0].value: "Function<void>" vs "Function&lt;void&gt;"

### 1× `$.children[N].spread`
- src/content/partials/api-shield/labels-add.mdx
  - $.children[3].spread: false vs true

### 1× `$.children[N].children[N].children[N].children[N].children[N].children[N].children`
- src/content/partials/cloudflare-one/warp/add-split-tunnels-route.mdx
  - $.children[2].children[0].children[0].children[4].children[1].children[0].children: array length 4 vs 7

### 1× `$.children[N].children[N].children[N].children[N].children`
- src/content/partials/cloudflare-one/warp/vpn-ip-traffic.mdx
  - $.children[2].children[1].children[1].children[0].children: array length 2 vs 3

### 1× `$.children[N].children[N].children[N].children[N].children[N].type`
- src/content/partials/networking-services/mconn/configure-connectors.mdx
  - $.children[38].children[0].children[1].children[0].children[1].type: "containerDirective" vs "paragraph"


## HAST mismatches — 33 unique pattern(s)

### 512× `$.children[N].attributes[N].value.value`
- src/content/docs/ai-gateway/demos.mdx
  - $.children[8].attributes[0].value.value: "[\n  \t\"reference-architecture\",\n  \t\"design-guide\",\n  \t\"reference-arch vs "[\n\t\t\"reference-architecture\",\n\t\t\"design-guide\",\n\t\t\"reference-arch
- src/content/docs/ai-gateway/evaluations/add-human-feedback-api.mdx
  - $.children[50].attributes[2].value.value: "{\n  \tfeedback: 1,\n  }" vs "{\n\t\tfeedback: 1,\n\t}"
- src/content/docs/ai-gateway/integrations/aig-workers-ai-binding.mdx
  - $.children[18].attributes[2].value.value: "{\n  \tcategory: \"hello-world\",\n  \ttype: \"Worker only\",\n  \tlang: \"Type vs "{\n\t\tcategory: \"hello-world\",\n\t\ttype: \"Worker only\",\n\t\tlang: \"Type
  … and 509 more

### 119× `$.children[N].children[N].children[N].attributes[N].value.value`
- src/content/docs/byoip/address-maps/setup.mdx
  - $.children[8].children[1].children[0].attributes[2].value.value: "{\n  description: \"Example address map\",\n  enabled: true,\n  ips: [\n    \"2 vs "{\n    description: \"Example address map\",\n    enabled: true,\n    ips: [\n 
- src/content/docs/byoip/service-bindings/magic-transit-with-cdn.mdx
  - $.children[10].children[3].children[3].attributes[2].value.value: "{\n\tpre_existing_product: \"Magic Transit\",\n\tadded_product: \"CDN\",\n}" vs "{\n\t\tpre_existing_product: \"Magic Transit\",\n\t\tadded_product: \"CDN\",\n\
- src/content/docs/cloudflare-one/access-controls/applications/http-apps/saas-apps/miro-saas.mdx
  - $.children[14].children[1].children[0].attributes[2].value.value: "{\n  \tone: \"**Security and Compliance** > **Authentication** > **Single sign- vs "{\n\t\tone: \"**Security and Compliance** > **Authentication** > **Single sign-
  … and 116 more

### 59× `$.children`
- src/content/docs/agents/guides/remote-mcp-server.mdx
  - $.children: array length 109 vs 111
- src/content/docs/agents/platform/limits.mdx
  - $.children: array length 13 vs 17
- src/content/docs/ai-gateway/usage/providers/anthropic.mdx
  - $.children: array length 25 vs 29
  … and 56 more

### 34× `$.children[N].children[N].children[N].children[N].properties.align`
- src/content/changelog/tunnel/2025-09-02-tunnel-networks-list-endpoints-new-default.mdx
  - $.children[12].children[1].children[1].children[1].properties.align: missing in actual
- src/content/docs/ai-gateway/usage/websockets-api/index.mdx
  - $.children[14].children[1].children[1].children[1].properties.align: missing in actual
- src/content/docs/analytics/graphql-api/sampling.mdx
  - $.children[26].children[1].children[1].children[1].properties.align: missing in actual
  … and 31 more

### 29× `$.children[N].children[N].properties.className`
- src/content/docs/analytics/graphql-api/tutorials/end-customer-analytics.mdx
  - $.children[4].children[6].properties.className: missing in expected
- src/content/docs/analytics/graphql-api/tutorials/querying-access-login-events.mdx
  - $.children[4].children[4].properties.className: missing in expected
- src/content/docs/analytics/graphql-api/tutorials/querying-email-routing.mdx
  - $.children[4].children[4].properties.className: missing in expected
  … and 26 more

### 22× `$.children[N].children[N].children[N].children[N].children[N].properties.className`
- src/content/changelog/workflows/2026-04-15-workflows-limits-raised.mdx
  - $.children[2].children[3].children[5].children[1].children[1].properties.className: missing in expected
- src/content/docs/cloudflare-one/insights/logs/index.mdx
  - $.children[10].children[3].children[5].children[11].children[1].properties.className: missing in expected
- src/content/docs/cloudflare-one/team-and-resources/devices/cloudflare-one-client/deployment/firewall.mdx
  - $.children[46].children[3].children[7].children[3].children[26].properties.className: missing in expected
  … and 19 more

### 15× `$.children[N].children[N].attributes[N].value.value`
- src/content/docs/cache/how-to/cache-response-rules/create-api.mdx
  - $.children[18].children[0].attributes[3].value.value: "{\n  rules: [\n    {\n      expression: 'http.request.uri.path.extension eq \"j vs "{\n    rules: [\n      {\n        expression: 'http.request.uri.path.extension 
- src/content/docs/cache/how-to/cache-rules/create-api.mdx
  - $.children[18].children[0].attributes[2].value.value: "{\n  rules: [\n    {\n      expression: '(http.host eq \"example.com\")',\n     vs "{\n    rules: [\n      {\n        expression: '(http.host eq \"example.com\")',
- src/content/docs/data-localization/metadata-boundary/get-started.mdx
  - $.children[22].children[1].attributes[2].value.value: "{\n  regions: \"eu\",\n  allow_out_of_region_access: false\n}" vs "{\n    regions: \"eu\",\n    allow_out_of_region_access: false\n  }"
  … and 12 more

### 13× `$.children[N].children[N].children[N].children[N].attributes[N].value.value`
- src/content/docs/artifacts/get-started/workers.mdx
  - $.children[16].children[0].children[1].children[5].attributes[2].value.value: "{\n   category: \"hello-world\",\n   type: \"Worker only\",\n   lang: \"TypeScr vs "{\n\t\tcategory: \"hello-world\",\n\t\ttype: \"Worker only\",\n\t\tlang: \"Type
- src/content/docs/cloudflare-for-platforms/cloudflare-for-saas/security/certificate-management/enforce-mtls.mdx
  - $.children[28].children[0].children[1].children[5].attributes[2].value.value: "{\n  \tssl: {\n  \t\tmethod: \"http\",\n  \t\ttype: \"dv\",\n  \t\tsettings: {\ vs "{\n\t\tssl: {\n\t\t\tmethod: \"http\",\n\t\t\ttype: \"dv\",\n\t\t\tsettings: {\
- src/content/docs/d1/tutorials/build-a-comments-api.mdx
  - $.children[10].children[0].children[1].children[5].attributes[2].value.value: "{\n   category: \"hello-world\",\n   type: \"Worker only\",\n   lang: \"TypeScr vs "{\n\t\tcategory: \"hello-world\",\n\t\ttype: \"Worker only\",\n\t\tlang: \"Type
  … and 10 more

### 13× `$.children[N].children[N].children`
- src/content/docs/browser-run/features/webmcp.mdx
  - $.children[72].children[2].children: array length 4 vs 2
- src/content/docs/cloudflare-one/access-controls/applications/http-apps/saas-apps/atlassian-saas.mdx
  - $.children[16].children[7].children: array length 3 vs 5
- src/content/docs/cloudflare-one/networks/connectors/cloudflare-tunnel/private-net/cloudflared/private-dns.mdx
  - $.children[8].children[3].children: array length 3 vs 9
  … and 10 more

### 13× `$.children[N].children[N].children[N].children[N].children[N].attributes[N].value.value`
- src/content/docs/cloudflare-one/access-controls/access-settings/independent-mfa.mdx
  - $.children[12].children[1].children[0].children[3].children[3].attributes[2].value.value: "{\n   \tauth_domain: \"your-team-name.cloudflareaccess.com\",\n   \tname: \"You vs "{\n\t\tauth_domain: \"your-team-name.cloudflareaccess.com\",\n\t\tname: \"Your 
- src/content/docs/cloudflare-one/access-controls/ai-controls/linked-apps.mdx
  - $.children[20].children[1].children[0].children[3].children[3].attributes[2].value.value: "{\n   name: \"Allow MCP server\",\n   decision: \"non_identity\",\n   include:  vs "{\n\t\tname: \"Allow MCP server\",\n\t\tdecision: \"non_identity\",\n\t\tinclud
- src/content/docs/cloudflare-one/access-controls/ai-controls/mcp-portals.mdx
  - $.children[90].children[1].children[0].children[3].children[3].attributes[2].value.value: "{\n   allow_code_mode: false,\n}" vs "{\n\t\tallow_code_mode: false,\n\t}"
  … and 10 more

### 11× `$.children[N].children`
- src/content/docs/ai-gateway/configuration/custom-costs.mdx
  - $.children[14].children: array length 3 vs 5
- src/content/docs/analytics/analytics-engine/sql-reference/string-functions.mdx
  - $.children[136].children: array length 7 vs 5
- src/content/docs/analytics/graphql-api/getting-started/querying-basics.mdx
  - $.children[26].children: array length 3 vs 5
  … and 8 more

### 10× `$.children[N].children[N].children[N].properties.className`
- src/content/docs/cloudflare-one/traffic-policies/identity-selectors.mdx
  - $.children[14].children[3].children[7].properties.className: missing in expected
- src/content/docs/dns/manage-dns-records/troubleshooting/stale-response.mdx
  - $.children[4].children[5].children[3].properties.className: missing in expected
- src/content/docs/durable-objects/concepts/what-are-durable-objects.mdx
  - $.children[14].children[1].children[3].properties.className: missing in expected
  … and 7 more

### 9× `$.children[N].children[N].children[N].children`
- src/content/changelog/waf/2025-08-11-waf-release.mdx
  - $.children[14].children[1].children[9].children: array length 3 vs 7
- src/content/changelog/waf/2025-12-18-waf-release.mdx
  - $.children[8].children[1].children[1].children: array length 3 vs 7
- src/content/changelog/waf/2026-01-12-waf-release.mdx
  - $.children[8].children[1].children[1].children: array length 3 vs 7
  … and 6 more

### 7× `$.children[N].children[N].children[N].children[N].children`
- src/content/changelog/browser-run/2025-07-28-br-pricing.mdx
  - $.children[8].children[3].children[3].children[7].children: array length 7 vs 6
- src/content/docs/cloudflare-one/traffic-policies/get-started/dns.mdx
  - $.children[30].children[0].children[0].children[1].children: array length 7 vs 3
- src/content/docs/learning-paths/secure-internet-traffic/build-dns-policies/create-policy.mdx
  - $.children[8].children[0].children[0].children[1].children: array length 7 vs 3
  … and 4 more

### 4× `$.children[N].children[N].children[N].children[N].children[N].children[N].value`
- src/content/docs/api-shield/management-and-monitoring/developer-portal.mdx
  - $.children[6].children[1].children[0].children[0].children[1].children[0].value: "\n" vs "Log in to the "
- src/content/partials/api-shield/set-up-session-identifiers.mdx
  - $.children[2].children[1].children[0].children[0].children[1].children[0].value: "\n" vs "Log in to the "
- src/content/partials/waf/dash-configure-all-rules.mdx
  - $.children[2].children[0].children[0].children[0].children[11].children[5].value: "props.rulesetName == \"Cloudflare Managed Ruleset\" && (\n   <Image src={cloudf vs "props.rulesetName == \"Cloudflare Managed Ruleset\" && (\n  <Image src={cloudfl
  … and 1 more

### 4× `$.children[N].children[N].children[N].children[N].value`
- src/content/partials/waf/managed-rules-browse-zone-new-nav.mdx
  - $.children[2].children[0].children[7].children[3].value: "props.rulesetName == \"Cloudflare Managed Ruleset\" && (\n   <Image src={cloudf vs "props.rulesetName == \"Cloudflare Managed Ruleset\" && (\n  <Image src={cloudfl
- src/content/partials/waf/managed-rules-browse-zone.mdx
  - $.children[4].children[0].children[7].children[3].value: "(props.rulesetName == \"\" || props.rulesetName == \"Cloudflare Managed Ruleset vs "(props.rulesetName == \"\" || props.rulesetName == \"Cloudflare Managed Ruleset
- src/content/partials/waf/managed-ruleset-configure-individual-rules.mdx
  - $.children[4].children[0].children[3].children[5].value: "(props.rulesetName == \"\" || props.rulesetName == \"Cloudflare Managed Ruleset vs "(props.rulesetName == \"\" || props.rulesetName == \"Cloudflare Managed Ruleset
  … and 1 more

### 3× `$.children[N].children[N].children[N].tagName`
- src/content/docs/cloudflare-one/access-controls/policies/index.mdx
  - $.children[88].children[1].children[3].tagName: "table" vs "p"
- src/content/docs/cloudflare-one/access-controls/policies/mfa-requirements.mdx
  - $.children[16].children[9].children[3].tagName: "table" vs "p"
- src/content/partials/cloudflare-one/access/modify-gateway-policy-precedence.mdx
  - $.children[2].children[0].children[1].tagName: "table" vs "p"

### 3× `$.children[N].tagName`
- src/content/docs/cloudflare-one/insights/analytics/shadow-it-discovery.mdx
  - $.children[26].tagName: "table" vs "p"
- src/content/partials/networking-services/prerequisites/router-vendor-guidelines-mss-settings-origin.mdx
  - $.children[4].tagName: "table" vs "p"
- src/content/partials/networking-services/tunnel-health/magic-tunnel-health-alerts.mdx
  - $.children[18].tagName: "table" vs "p"

### 3× `$.children[N].children[N].children[N].children[N].children[N].children[N].children`
- src/content/docs/cloudflare-one/networks/connectors/cloudflare-tunnel/private-net/cloudflared/tunnel-virtual-networks.mdx
  - $.children[20].children[0].children[1].children[1].children[2].children[1].children: array length 9 vs 16
- src/content/docs/ssl/edge-certificates/custom-certificates/uploading.mdx
  - $.children[14].children[0].children[1].children[11].children[3].children[1].children: array length 3 vs 2
- src/content/partials/cloudflare-one/warp/add-split-tunnels-route.mdx
  - $.children[2].children[0].children[0].children[9].children[3].children[0].children: array length 3 vs 7

### 3× `$.children[N].children[N].children[N].children[N].properties.className`
- src/content/docs/dns/manage-dns-records/reference/wildcard-dns-records.mdx
  - $.children[32].children[3].children[3].children[1].properties.className: missing in expected
- src/content/docs/workers/static-assets/headers.mdx
  - $.children[6].children[7].children[3].children[5].properties.className: missing in expected
- src/content/docs/zaraz/get-started.mdx
  - $.children[8].children[9].children[1].children[3].properties.className: missing in expected

### 3× `$.children[N].children[N].children[N].properties.alt`
- src/content/docs/style-guide/components/index.mdx
  - $.children[14].children[0].children[0].properties.alt: "DashButton component\nexample" vs "DashButton component example"
- src/content/partials/networking-services/mnm-magic-transit-integration.mdx
  - $.children[16].children[0].children[0].properties.alt: "You can create rules to activate Magic Transit automatically, to protect your I vs "You can create rules to activate Magic Transit automatically, to protect your I
- src/content/partials/style-guide/llms-txt.mdx
  - $.children[14].children[0].children[0].properties.alt: "Page options\nbutton" vs "Page options button"

### 3× `$.children[N].children[N].children[N].value`
- src/content/partials/api-shield/labels-add.mdx
  - $.children[4].children[7].children[1].value: " props.labelName ? (\n  <>\n      <p>Add the <code>{props.labelName}</code> lab vs " props.labelName ? (\n <>\n     <p>Add the <code>{props.labelName}</code> label
- src/content/partials/cloudflare-one/access/self-hosted-app/create-app.mdx
  - $.children[2].children[9].children[5].value: "\nprops.private && (\n   <p> \t\t\tIf the application is non-HTTPS or you do no vs "\nprops.private && (\n  <p> \t\t\tIf the application is non-HTTPS or you do not
- src/content/partials/networking-services/routing/bgp-config-steps.mdx
  - $.children[2].children[3].children[3].value: " props.magicWord !== \"Magic Transit\" && (\n   <>\n   <Aside type=\"note\">Mul vs " props.magicWord !== \"Magic Transit\" && (\n  <>\n  <Aside type=\"note\">Multi

### 2× `$.children[N].children[N].children[N].children[N].children[N].children[N].attributes[N].value.value`
- src/content/docs/d1/get-started.mdx
  - $.children[26].children[0].children[0].children[0].children[1].children[5].attributes[2].value.value: "{\n   category: \"hello-world\",\n   type: \"Worker only\",\n   lang: \"TypeScr vs "{\n \tcategory: \"hello-world\",\n \ttype: \"Worker only\",\n \tlang: \"TypeScr
- src/content/docs/kv/get-started.mdx
  - $.children[26].children[0].children[1].children[0].children[1].children[5].attributes[2].value.value: "{\n   category: \"hello-world\",\n   type: \"Worker only\",\n   lang: \"TypeScr vs "{\n \tcategory: \"hello-world\",\n \ttype: \"Worker only\",\n \tlang: \"TypeScr

### 2× `$.children[N].children[N].children[N].type`
- src/content/docs/email-security/migrate-to-email-security.mdx
  - $.children[10].children[1].children[0].type: "element" vs "text"
- src/content/partials/cloudflare-one/access/add-mtls-cert.mdx
  - $.children[0].children[7].children[7].type: "mdxFlowExpression" vs "element"

### 1× `$.children[N].children[N].value`
- src/content/changelog/containers/2025-11-21-new-cpu-pricing.mdx
  - $.children[12].children[0].value: "CPU cost would have been: " vs "CPU cost would have been: **"

### 1× `$.children[N].children[N].children[N].children[N].type`
- src/content/docs/analytics/graphql-api/getting-started/authentication/index.mdx
  - $.children[6].children[1].children[1].children[1].type: "mdxJsxFlowElement" vs "element"

### 1× `$.children[N].children[N].attributes[N].value`
- src/content/docs/billing/index.mdx
  - $.children[26].children[4].attributes[0].value: "Resolve \"you cannot modify this subscription\"" vs "Resolve &quot;you cannot modify this subscription&quot;"

### 1× `$.children[N].children[N].children[N].children[N].children[N].children[N].properties.className`
- src/content/docs/bots/plans/biz-and-ent.mdx
  - $.children[6].children[1].children[1].children[1].children[0].children[1].properties.className: missing in expected

### 1× `$.children[N].children[N].children[N].children[N].children[N].children[N].children[N].attributes[N].value.value`
- src/content/docs/cloudflare-one/networks/connectors/cloudflare-tunnel/configure-tunnels/remote-tunnel-permissions.mdx
  - $.children[16].children[1].children[3].children[1].children[0].children[3].children[3].attributes[2].value.value: "{\n  \t\tname: \"Example tunnel\",\n  \t\ttunnel_secret: \"AQIDBAUGBwgBAgMEBQYH vs "{\n\t\t\tname: \"Example tunnel\",\n\t\t\ttunnel_secret: \"AQIDBAUGBwgBAgMEBQYH

### 1× `$.children[N].children[N].properties.alt`
- src/content/docs/support/third-party-software/content-management-system-cms/how-do-i-enable-http2-server-push-in-wordpress.mdx
  - $.children[2].children[0].properties.alt: "Old URL: https://support.cloudflare.com/hc/en-us/article_attachments/1150057333 vs "Old URL: https://support.cloudflare.com/hc/en-us/article_attachments/1150057333

### 1× `$.children[N].children[N].children[N].children[N].children[N].children[N].properties.align`
- src/content/docs/waf/reference/legacy/old-rate-limiting/upgrade.mdx
  - $.children[6].children[9].children[3].children[1].children[1].children[3].properties.align: missing in actual

### 1× `$.children[N].children[N].children[N].children[N].children[N].attributes[N].value`
- src/content/docs/workers/runtime-apis/html-rewriter.mdx
  - $.children[56].children[25].children[1].children[0].children[1].attributes[0].value: "Function<void>" vs "Function&lt;void&gt;"

### 1× `$.children[N].children[N].children[N].children[N].children[N].properties.align`
- src/content/partials/dns/proxy-status-dns-table.mdx
  - $.children[2].children[1].children[1].children[1].children[1].properties.align: missing in actual

