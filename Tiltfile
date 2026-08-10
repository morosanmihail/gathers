local_resource(
    'server',
    serve_cmd='cargo run --bin server',
    deps=[
        'server/src',
        'models/src',
        'retrieval/src',
        'persistence/src',
        'Cargo.toml',
        'Cargo.lock',
    ],
    links=[link('http://localhost:5234', 'API')],
    labels=['backend'],
)

local_resource(
    'webui2',
    serve_cmd='cd webui2 && npm run dev',
    deps=['webui2/src', 'webui2/package.json', 'webui2/svelte.config.js', 'webui2/vite.config.ts'],
    resource_deps=['server'],
    links=[link('http://localhost:5173', 'UI v2')],
    labels=['frontend'],
)
