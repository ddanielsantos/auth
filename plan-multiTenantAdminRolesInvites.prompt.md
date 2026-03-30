## Plan: Multi-tenant Admin com Roles e Invites

Modelar autorização multi-tenant em camadas: primeiro o domínio (membros, roles, convites), depois enforcement nas rotas existentes (`/admin/*`), e por fim UX/API de gestão de membros. Isso permite que um admin participe de múltiplas orgs/projetos com permissões claras (`owner`/`admin`), mantém compatibilidade com o MVP em `docs/TODO.md`, e evita vazamento de dados globais hoje visíveis em métricas/logs.

### Steps
1. Definir matriz de permissões `owner`/`admin` em `docs/TODO.md` e `docs/ERD.md` com regras por recurso.
2. Criar migrations de tenancy (`admin_org_memberships`, `admin_project_memberships`, `admin_invites`) em `infra/migrations/` com índices e constraints.
3. Evoluir JWT/contexto de autorização em `src/jwt.rs` e `src/admin.rs` para extrair `admin_id` e escopo ativo.
4. Introduzir guardas de acesso reutilizáveis em `src/admin/authorization.rs` e aplicar nas rotas de `src/admin/router.rs`.
5. Implementar fluxo de convites (`POST /admin/invites`, `POST /admin/invites/{id}/accept`, `POST /admin/invites/{id}/decline`) em `src/admin/router/`.
6. Refatorar `GET /admin/metrics` e `GET /admin/logs` em `src/admin/router.rs` para filtrar por memberships org/projeto.

### Further Considerations
1. Escopo ativo no token/cabeçalho: Opção A `X-Org-Id`; Opção B `X-Project-Id`; Opção C ambos com prioridade explícita.
2. Membership em projeto herda org automaticamente? Opção A herda sempre; Opção B explícito por projeto; Opção C híbrido com override.
3. Convite por `username` ou email externo? Opção A `username` (simples); Opção B email+token (mais robusto); Opção C suportar ambos.

