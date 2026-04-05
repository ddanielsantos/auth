```mermaid
erDiagram
    IDENTITIES ||--o{ LOGIN_METHODS : "autentica_via"
    IDENTITIES ||--o{ USER_ACCOUNTS : "pertence a"
    ORGANIZATIONS ||--o{ PROJECTS : "possui"
    ORGANIZATIONS ||--o{ ADMIN_ORG_MEMBERSHIPS : "tem membros"
    ORGANIZATIONS ||--o{ ADMIN_INVITES : "escopo"
    PROJECTS ||--o{ APPLICATIONS : "agrupa"
    PROJECTS ||--o{ USER_ACCOUNTS : "contém"
    PROJECTS ||--o{ ADMIN_PROJECT_MEMBERSHIPS : "tem membros"
    PROJECTS ||--o{ ADMIN_INVITES : "escopo"
    APPLICATIONS ||--o{ PERMISSIONS : "define"
    APPLICATIONS ||--o{ AUTH_EVENTS : "gera eventos"
    ADMIN_USERS ||--o{ ADMIN_ORG_MEMBERSHIPS : "participa"
    ADMIN_USERS ||--o{ ADMIN_PROJECT_MEMBERSHIPS : "participa"
    ADMIN_USERS ||--o{ ADMIN_INVITES : "envia"
    ADMIN_USERS ||--o{ AUTH_EVENTS : "autenticação"
    USER_ACCOUNTS ||--o{ ACCOUNT_SCOPES : "possui"
    USER_ACCOUNTS ||--o{ AUTH_EVENTS : "tenta autenticar"
    PERMISSIONS ||--o{ ACCOUNT_SCOPES : "atribuída a"

    IDENTITIES {
        uuid id PK
        timestamp created_at
        boolean is_active
    }

    LOGIN_METHODS {
        uuid id PK
        uuid identity_id FK
        string method_type "Ex: email, phone, username, sub_google"
        string identifier "O valor real: joao@mail.com, 11999..., @jao"
        string password_hash "Opcional (nulo para login social)"
        boolean is_verified
        constraint "UNIQUE(method_type, identifier)"
    }

    ORGANIZATIONS {
        uuid id PK
        string name
        timestamp created_at
        constraint "UNIQUE(name)"
    }

    PROJECTS {
        uuid id PK
        uuid org_id FK
        string name
        boolean shared_identity_context "Se true, SSO entre apps do projeto"
        constraint "UNIQUE(org_id, name)"
    }

    APPLICATIONS {
        uuid id PK
        uuid project_id FK
        uuid client_id UK
        string name
        string client_secret_hash
        text redirect_uris "Array de URIs permitidas"
    }

    USER_ACCOUNTS {
        uuid id PK
        uuid identity_id FK
        uuid project_id FK
        jsonb local_profile_data "JSON com nome, foto, etc"
        constraint "UNIQUE(identity_id, project_id)"
    }
    
    PERMISSIONS {
        uuid id PK
        uuid app_id FK
        string name "ex: files:read"
        string description
        constraint "UNIQUE(app_id, name)"
    }

    ACCOUNT_SCOPES {
        uuid account_id FK
        uuid permission_id FK
        constraint "PRIMARY KEY(account_id, permission_id)"
    }

    ADMIN_USERS {
        uuid id PK
        string username UK
        string password_hash
    }

    ADMIN_ORG_MEMBERSHIPS {
        uuid id PK
        uuid admin_user_id FK
        uuid org_id FK
        string role "owner|admin"
        timestamp created_at
        constraint "UNIQUE(admin_user_id, org_id)"
    }

    ADMIN_PROJECT_MEMBERSHIPS {
        uuid id PK
        uuid admin_user_id FK
        uuid project_id FK
        string role "owner|admin"
        timestamp created_at
        constraint "UNIQUE(admin_user_id, project_id)"
    }

    ADMIN_INVITES {
        uuid id PK
        uuid invited_by_admin_user_id FK
        uuid org_id FK "nullable, exclusivo com project_id"
        uuid project_id FK "nullable, exclusivo com org_id"
        string invitee_username
        string role "owner|admin"
        string status "pending|accepted|declined|expired|revoked"
        timestamp expires_at
        timestamp responded_at
        timestamp created_at
        constraint "CHECK((org_id IS NOT NULL AND project_id IS NULL) OR ...)"
    }

    AUTH_EVENTS {
        uuid id PK
        string event_type
        boolean success
        string route
        uuid admin_user_id FK "nullable, ON DELETE SET NULL"
        uuid application_id FK "nullable, ON DELETE SET NULL"
        string application_name "redundância para auditoria (caso app seja deletada)"
        string identifier
        string ip_address
        integer http_status
        timestamp occurred_at
        constraint "INDEX(occurred_at DESC)"
        constraint "INDEX(success, occurred_at DESC)"
        constraint "INDEX(route, occurred_at DESC)"
    }
```