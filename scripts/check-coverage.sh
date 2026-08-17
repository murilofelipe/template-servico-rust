#!/bin/bash
set -e

echo "=> Rodando testes e verificando cobertura (Threshold: 60%)..."
if make coverage-check; then
    echo "=> Cobertura aceitável! Abrindo relatório..."
    if command -v xdg-open > /dev/null; then
        xdg-open tarpaulin-report.html
    elif command -v open > /dev/null; then
        open tarpaulin-report.html
    else
        echo "=> Relatório gerado em tarpaulin-report.html"
    fi
else
    echo "=> ERRO: Cobertura de testes abaixo do limite (60%)!"
    exit 1
fi
