# Endpoints Faltantes — Admin Frontend

> Análise baseada nas páginas do frontend (`auth_web/src/pages/`) vs. rotas existentes em `src/admin/router.rs`.  
> Escopo: **apenas endpoints de uso do administrador** (console `Sovereign Vault`).

---

## Rotas Existentes (referência)

| Método | Rota | Status |
|--------|------|--------|
| `POST` | `/admin/register` | ✅ Implementado |
| `POST` | `/admin/login` | ✅ Implementado |
| `POST` | `/admin/organizations` | ✅ Implementado |
| `POST` | `/admin/projects` | ✅ Implementado |
| `POST` | `/admin/applications` | ✅ Implementado |
| `PUT`  | `/admin/applications/{app_id}/scopes` | ✅ Implementado |
| `GET`  | `/admin/metrics` | ✅ Implementado |
| `GET`  | `/admin/logs` | ✅ Implementado |

---

## Endpoints Faltantes

### 1. Perfil do Admin Autenticado

**`GET /admin/me`**

Necessário para exibir o nome/username do admin na `TopNav` (atualmente hardcoded como `"Admin"`).

**Response:**
```json
{
  "id": "uuid",
  "username": "string"
}
```

---

### 2. Dashboard — Métricas Reais

**`GET /admin/metrics`**

A rota agora retorna payload real com `active_applications`, `total_requests_24h` e `failed_attempts_24h` com base em `auth_events` (janela de 24h). `uptime_percentage` permanece fixo temporariamente até integração com observabilidade externa.

**Response esperada:**
```json
{
  "total_requests_24h": 1200000,
  "active_applications": 42,
  "failed_attempts_24h": 128,
  "uptime_percentage": 99.9
}
```

> `total_requests_24h` e `failed_attempts_24h` dependem de escrita consistente em `auth_events` nos fluxos de autenticação.

---

### 3. Dashboard — Log de Atividade Recente

**`GET /admin/logs`**

Implementado com paginação por `limit`/`offset`, retornando dados de `auth_events` em ordem decrescente de `occurred_at`.

**Query params sugeridos:**
- `limit` (default: 20)
- `offset` ou `cursor`

**Response:**
```json
{
  "items": [
    {
      "id": "uuid",
      "event_type": "login_success | login_failed | token_expired | mfa_validated",
      "identifier": "user@email.com",
      "application_id": "uuid",
      "application_name": "string",
      "ip_address": "string",
      "occurred_at": "ISO8601"
    }
  ],
  "total": 1024
}
```

> `application_name` é resolvido via `auth_events -> applications -> projects`.

---

### 4. Listagem de Organizações

**`GET /admin/organizations`**

Necessário para que o modal "Criar Projeto" possa oferecer seleção de uma organização já existente (além de criar uma nova). Atualmente o fluxo sempre cria uma org nova.

**Response:**
```json
[
  {
    "id": "uuid",
    "name": "string",
    "created_at": "ISO8601"
  }
]
```

---

### 5. Listagem de Projetos

**`GET /admin/projects`**

A página `/projects` usa estado local otimista — a lista começa vazia a cada refresh. Sem este endpoint, projetos criados anteriormente desaparecem ao recarregar a página.

**Response:**
```json
[
  {
    "id": "uuid",
    "name": "string",
    "org_id": "uuid",
    "org_name": "string",
    "shared_identity_context": false,
    "application_count": 3
  }
]
```

> Requer `JOIN` com `organizations` para retornar `org_name`.

---

### 6. Detalhes de um Projeto

**`GET /admin/projects/{project_id}`**

A página `/projects/:id` não consegue exibir o nome nem metadados do projeto — esses dados não estão disponíveis ao navegar diretamente para a rota.

**Response:**
```json
{
  "id": "uuid",
  "name": "string",
  "org_id": "uuid",
  "org_name": "string",
  "shared_identity_context": false,
  "created_at": "ISO8601"
}
```

---

### 7. Listagem de Aplicações de um Projeto

**`GET /admin/projects/{project_id}/applications`**

A página de detalhes do projeto usa estado local otimista para as aplicações. Sem este endpoint, aplicações criadas anteriormente não aparecem ao recarregar.

**Response:**
```json
[
  {
    "id": "uuid",
    "client_id": "uuid",
    "redirect_uris": ["https://app.com/callback"],
    "scopes": [
      {
        "id": "uuid",
        "name": "files:read",
        "description": "Permissão de leitura de arquivos"
      }
    ]
  }
]
```

> `client_secret_hash` **não** deve ser retornado. O `raw_client_secret` só é exposto uma vez na criação (`POST /admin/applications`).

---

### 8. Listagem de Administradores

**`GET /admin/users`**

A página `/administrators` usa estado local otimista. Sem este endpoint, admins criados anteriormente não aparecem ao recarregar.

**Response:**
```json
[
  {
    "id": "uuid",
    "username": "string",
    "created_at": "ISO8601"
  }
]
```

> `password_hash` **nunca** deve ser retornado.

---

## Endpoints de Deleção (Soft Delete / EL)

Conforme especificado no `TODO.md`, a deleção de recursos do admin é **Exclusão Lógica (EL)** — adicionar campo `deleted_at` ou `is_active` nas tabelas.

| Método | Rota | Página que consome |
|--------|------|--------------------|
| `DELETE` | `/admin/organizations/{id}` | — (futuro) |
| `DELETE` | `/admin/projects/{id}` | `/projects` — botão de delete no card |
| `DELETE` | `/admin/applications/{id}` | `/projects/:id` — botão de delete na aplicação |
| `DELETE` | `/admin/users/{id}` | `/administrators` — botão de remover admin |

> Para `organizations`: deleção em cascata deve cobrir projetos e aplicações filhas, ou ser bloqueada se houver dependências.

---

## Endpoints de Atualização (PATCH)

Necessários para edição inline dos recursos. Não há formulários de edição no frontend atual, mas os botões de ação nos cards sugerem essa intenção futura.

| Método | Rota | Campos editáveis |
|--------|------|-----------------|
| `PATCH` | `/admin/projects/{id}` | `name`, `shared_identity_context` |
| `PATCH` | `/admin/applications/{id}` | `redirect_uris` |

---

## Resumo de Prioridade

| Prioridade | Endpoint | Motivo |
|---|---|---|
| 🔴 P0 | `GET /admin/projects` | Dados somem ao recarregar |
| 🔴 P0 | `GET /admin/projects/{id}/applications` | Dados somem ao recarregar |
| 🔴 P0 | `GET /admin/users` | Dados somem ao recarregar |
| 🟠 P1 | `GET /admin/me` | Nome do admin hardcoded na UI |
| 🟠 P1 | `GET /admin/projects/{id}` | Título da página sem dados reais |
| 🟡 P2 | `GET /admin/organizations` | Fluxo "criar projeto" sempre cria org nova |
| 🟡 P2 | `DELETE /admin/projects/{id}` | EL prevista no roadmap |
| 🟡 P2 | `DELETE /admin/applications/{id}` | EL prevista no roadmap |
| 🟡 P2 | `DELETE /admin/users/{id}` | EL prevista no roadmap |
| 🟢 P3 | `PATCH /admin/projects/{id}` | Edição inline (futuro) |
| 🟢 P3 | `PATCH /admin/applications/{id}` | Edição inline (futuro) |
