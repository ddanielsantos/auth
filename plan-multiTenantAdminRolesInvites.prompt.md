## Plan: Multi-tenant Admin com Roles e Invites

Modelar autorização multi-tenant em camadas: primeiro o domínio (membros, roles, convites), depois enforcement nas rotas existentes (`/admin/*`), e por fim UX/API de gestão de membros. Isso permite que um admin participe de múltiplas orgs/projetos com permissões claras (`owner`/`admin`), mantém compatibilidade com o MVP em `docs/TODO.md`, e evita vazamento de dados globais hoje visíveis em métricas/logs.

### Decisão de escopo: URL hierárquica (Option C)

O escopo ativo (org ou projeto) é passado **na própria URL**, seguindo hierarquia REST:

```
/admin/orgs/{org_id}/projects
/admin/orgs/{org_id}/projects/{project_id}/applications
/admin/orgs/{org_id}/logs
/admin/orgs/{org_id}/projects/{project_id}/logs
```

Isso torna o escopo visível em logs, inequívoco nos docs do Swagger, e consistente com o padrão já usado em `/{app_id}/scopes`. Ver `docs/MISSING_ENDPOINTS.md` para o mapeamento completo de rotas antigas → novas.

### Steps
1. Refatorar rotas existentes de flat (`/admin/projects`) para hierárquicas (`/admin/orgs/{org_id}/projects`) em `src/admin/router.rs`.
2. Criar `src/admin/authorization.rs` com extractors reutilizáveis que validam membership por org ou projeto antes de cada handler.
3. Ao criar org (`POST /admin/orgs`), inserir automaticamente o caller como `owner` em `admin_org_memberships`.
4. Escopar `GET /admin/orgs/{org_id}/metrics` e `GET /admin/orgs/{org_id}/logs` para filtrar por org/projeto.
5. Implementar fluxo de convites: `POST /admin/orgs/{org_id}/invites`, `POST /admin/orgs/{org_id}/projects/{project_id}/invites`, e as ações `accept`/`decline`/`revoke` em `/admin/invites/{id}/*`.
6. Adicionar endpoints de leitura faltantes: `GET /admin/me`, `GET /admin/orgs`, `GET /admin/orgs/{org_id}/projects`, `GET /admin/orgs/{org_id}/projects/{project_id}/applications`, `GET /admin/users`.

### Further Considerations
1. Membership em projeto herda org automaticamente? Opção A herda sempre; Opção B explícito por projeto; Opção C híbrido com override.
2. Convite por `username` ou email externo? MVP usa `username` (simples).
3. Soft delete: adicionar `deleted_at` nas tabelas org, project, application antes ou durante esta feature?

