# Como contribuir

O código é aberto. Mudanças da equipe e da comunidade passam pelo mesmo processo de revisão: branch, pull request, CI e aprovação antes de entrar em main.

## Equipe interna

1. Atualize sua cópia de main.
2. Crie uma branch com um nome ligado à mudança.
3. Faça commits pequenos e explique o motivo de cada mudança.
4. Abra um pull request com o modelo do repositório.
5. Resolva comentários e aguarde as verificações automáticas e a revisão dos responsáveis.

## Comunidade externa

1. Abra uma issue para discutir uma alteração grande ou um problema ainda sem reprodução.
2. Crie um fork e uma branch no seu fork.
3. Faça mudanças pequenas, inclua os testes aplicáveis e abra um pull request para main.
4. Responda aos comentários e atualize o PR até que as verificações estejam verdes.

O commit é a unidade de histórico; o pull request é a proposta revisável que reúne um ou mais commits. Ninguém precisa criar uma pasta própria para o código. O GitHub registra autoria e revisão no PR.

## Mudanças de GPU

Relatórios de compatibilidade usam `reports/hardware/_TEMPLATE.md`. Para uma mudança de shader ou GPU, registre capacidades consultadas e o hardware efetivamente testado. Um teste em uma GPU não deve ser apresentado como resultado em outra arquitetura. Testes em hardware físico podem depender de runner dedicado e aprovação da equipe.

## Verificações locais

Execute antes de abrir o PR:

    cargo fmt --all -- --check
    cargo clippy --workspace --locked
    cargo test --workspace --locked

Clippy reports a known baseline of warnings in the inherited GPU prototype. The CI keeps these visible but does not treat them as errors yet; Rust compile errors and failing tests still block a PR. We can restore `-D warnings` after the baseline warnings are resolved.

The GitHub-hosted Windows runner compiles the `sixm-gpu` and `sixm-shader` test targets but does not execute their device-dependent tests: virtual adapters do not provide a stable substitute for the project's supported GPU environment, and failures vary by adapter. The full suite runs in `.github/workflows/gpu-validation.yml` on a trusted, configured GPU runner. Tests in the other workspace packages run in regular CI. A developer with a suitable local adapter can run the complete `cargo test --workspace --locked` suite.

## Arquivos que não entram

Não inclua executáveis, pacotes, dumps, arquivos de jogo, assets proprietários, caches, chaves, tokens, caminhos pessoais ou logs brutos. Os testes devem usar código e fixtures sintéticas redistribuíveis.
