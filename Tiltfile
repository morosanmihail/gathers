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
    'webui',
    serve_cmd='cd webui && npm start',
    deps=['webui/package.json'],
    resource_deps=['server'],
    links=[link('http://localhost:3000', 'UI')],
    labels=['frontend'],
)
