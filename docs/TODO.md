UF  = usuario final
UA  = usuario admin
org = organizacao
pr  = projetos
app = aplicacao
EL  = exclusao logica
EB  = exclusao bruta

# MVP

✅ UA cria sua conta no sistema 
🕜 UA loga no sistema
UA cria uma org e automaticamente obtem o role de owner para aquela org
    role owner -> 
        EB, invite de outros admins + permissoes de admin
        pode passar o role de owner pra outro admin
        pode aceitar ou recusar EL
    role admin -> 
        EL, editar, ler org e suas dependencias (pr, app)
        criar pr e app
EL pode ser aplicada em org, pr, app
todas as acoes de escrita dos UA sao logadas em um historico/event sourcing

## Multi-tenant admin (MVP)

### Roles e escopos

- `owner` e `admin` sao os unicos roles do MVP.
- Membership pode existir em `org` e em `pr`.
- `owner` de org herda acesso aos projetos daquela org.
- Membership de projeto nao cria automaticamente membership de org.

### Matriz de permissoes (MVP)

| Recurso/acao | owner (org) | admin (org) | owner (pr) | admin (pr) |
|---|---:|---:|---:|---:|
| Ver org | ✅ | ✅ | ✅ (somente org do pr) | ✅ (somente org do pr) |
| Criar projeto na org | ✅ | ✅ | ❌ | ❌ |
| Convidar admins para org | ✅ | ✅ | ❌ | ❌ |
| Criar aplicacao no projeto | ✅ | ✅ | ✅ | ✅ |
| Gerenciar scopes da aplicacao | ✅ | ✅ | ✅ | ✅ |
| Ler metrics | ✅ (escopo visivel) | ✅ (escopo visivel) | ✅ (somente pr) | ✅ (somente pr) |
| Ler logs | ✅ (escopo visivel) | ✅ (escopo visivel) | ✅ (somente pr) | ✅ (somente pr) |
| Remover owner | ✅ (nao pode remover ultimo owner) | ❌ | ✅ (nao pode remover ultimo owner) | ❌ |

### Invites

- Estado: `pending -> accepted | declined | expired | revoked`.
- Convite referencia `org` ou `pr` (nunca ambos ao mesmo tempo).
- Convite define o role de destino (`owner`/`admin`).
- Convite do MVP usa `username` como destinatario.

### Regra de visibilidade para `/admin/metrics` e `/admin/logs`

- Se escopo ativo for org, mostrar somente dados de projetos/aplicacoes daquela org.
- Se escopo ativo for pr, mostrar somente dados daquele pr.
- Sem escopo explicito, usar uniao dos escopos em que o admin possui membership.

