# Endpoints Faltantes — Admin Frontend

> Análise baseada nas páginas do frontend (`auth_web/src/pages/`) vs. rotas existentes em `src/admin/router.rs`.  
> Escopo: **apenas endpoints de uso do administrador** (console `Sovereign Vault`).  
> **Nota:** As rotas foram redesenhadas para usar hierarquia de URL (Option C) como escopo explícito.
> Ver `plan-multiTenantAdminRolesInvites.prompt.md` para contexto da decisão.

---

## Rotas Existentes (a serem refatoradas para hierarquia URL)

| Método | Rota atual (flat) | Rota nova (hierárquica) | Status |
|--------|-------------------|------------------------|--------|
| `POST` | `/admin/register` | `/admin/register` *(sem mudança)* | ✅ |
| `POST` | `/admin/login` | `/admin/login` *(sem mudança)* | ✅ |
| `POST` | `/admin/organizations` | `/admin/orgs` | 🔄 renomear |
| `POST` | `/admin/projects` | `/admin/orgs/{org_id}/projects` | 🔄 mover + guardar |
| `POST` | `/admin/applications` | `/admin/orgs/{org_id}/projects/{project_id}/applications` | 🔄 mover + guardar |
| `PUT`  | `/admin/applications/{app_id}/scopes` | `/admin/orgs/{org_id}/projects/{project_id}/applications/{app_id}/scopes` | 🔄 mover |
| `GET`  | `/admin/metrics` | `/admin/orgs/{org_id}/metrics` e `/admin/orgs/{org_id}/projects/{project_id}/metrics` | 🔄 escopar |
| `GET`  | `/admin/logs` | `/admin/orgs/{org_id}/logs` e `/admin/orgs/{org_id}/projects/{project_id}/logs` | 🔄 escopar |

---

## Endpoints Faltantes

### Auth / Perfil

**`GET /admin/me`** — P1  
Necessário para exibir o username do admin na `TopNav`.

```json
{ "id": "uuid", "username": "string" }
```

---

### Organizações

**`GET /admin/orgs`** — P2  
Lista orgs do admin autenticado (via `admin_org_memberships`).

```json
[{ "id": "uuid", "name": "string", "role": "owner|admin", "created_at": "ISO8601" }]
```

**`GET /admin/orgs/{org_id}`** — P1  
Detalhes de uma org (requer membership).

**`DELETE /admin/orgs/{org_id}`** — P2 (soft delete)

---

### Projetos

**`GET /admin/orgs/{org_id}/projects`** — P0  
Lista projetos da org. Requer `owner|admin` da org ou membership direta no projeto.

```json
[{ "id": "uuid", "name": "string", "shared_identity_context": false, "application_count": 3 }]
```

**`GET /admin/orgs/{org_id}/projects/{project_id}`** — P1  
Detalhes de um projeto. Requer membership.

**`PATCH /admin/orgs/{org_id}/projects/{project_id}`** — P3  
Editar `name` / `shared_identity_context`.

**`DELETE /admin/orgs/{org_id}/projects/{project_id}`** — P2 (soft delete)

---

### Aplicações

**`GET /admin/orgs/{org_id}/projects/{project_id}/applications`** — P0  
Lista aplicações do projeto. Requer membership.

```json
[{ "id": "uuid", "name": "string", "client_id": "uuid", "redirect_uris": ["..."], "scopes": [] }]
```

> `client_secret_hash` nunca é retornado. `raw_client_secret` só é exposto uma vez na criação.

**`DELETE /admin/orgs/{org_id}/projects/{project_id}/applications/{app_id}`** — P2 (soft delete)

**`PATCH /admin/orgs/{org_id}/projects/{project_id}/applications/{app_id}`** — P3  
Editar `redirect_uris`.

---

### Convites

**`POST /admin/orgs/{org_id}/invites`** — P1  
Convidar admin para a org. Requer `owner|admin` da org.

```json
{ "invitee_username": "string", "role": "owner|admin" }
```

**`POST /admin/orgs/{org_id}/projects/{project_id}/invites`** — P1  
Convidar admin para o projeto. Requer `owner|admin` do projeto ou da org pai.

**`POST /admin/invites/{id}/accept`** — P1  
Invitee aceita o convite; insere membership.

**`POST /admin/invites/{id}/decline`** — P1

**`POST /admin/invites/{id}/revoke`** — P1  
Issuer revoga convite pendente. Requer `owner|admin` do escopo original.

---

### Administradores

**`GET /admin/users`** — P0  
Lista admins visíveis (membros das orgs/projetos do caller).

```json
[{ "id": "uuid", "username": "string", "created_at": "ISO8601" }]
```

> `password_hash` nunca é retornado.

---

### Métricas e Logs (escopados)

**`GET /admin/orgs/{org_id}/metrics`** — P1  
Métricas filtradas pela org (todos os projetos e apps da org).

**`GET /admin/orgs/{org_id}/projects/{project_id}/metrics`** — P2  
Métricas filtradas pelo projeto específico.

**`GET /admin/orgs/{org_id}/logs`** — P1  
Logs filtrados pela org.

**`GET /admin/orgs/{org_id}/projects/{project_id}/logs`** — P2  
Logs filtrados pelo projeto específico. Suporta cursor pagination.

---

## Resumo de Prioridade

| Prioridade | Endpoint | Motivo |
|---|---|---|
| 🔴 P0 | `GET /admin/orgs/{org_id}/projects` | Dados somem ao recarregar |
| 🔴 P0 | `GET /admin/orgs/{org_id}/projects/{project_id}/applications` | Dados somem ao recarregar |
| 🔴 P0 | `GET /admin/users` | Dados somem ao recarregar |
| 🟠 P1 | `GET /admin/me` | Nome do admin hardcoded na UI |
| 🟠 P1 | `GET /admin/orgs/{org_id}` | Título da página sem dados reais |
| 🟠 P1 | `GET /admin/orgs/{org_id}/projects/{project_id}` | Título da página sem dados reais |
| 🟠 P1 | Invite endpoints | Fluxo de convites completo |
| 🟠 P1 | `GET /admin/orgs/{org_id}/metrics` e `/logs` | Métricas/logs escopados |
| 🟡 P2 | `GET /admin/orgs` | Seleção de org no modal |
| 🟡 P2 | DELETE endpoints | EL prevista no roadmap |
| 🟡 P2 | Metrics/logs por projeto | Granularidade adicional |
| 🟢 P3 | PATCH endpoints | Edição inline (futuro) |



