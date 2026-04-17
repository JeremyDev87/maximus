# Maximus

[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](https://github.com/JeremyDev87/maximus/blob/master/LICENSE)

<p align="center">
  <a href="https://github.com/JeremyDev87/maximus/blob/master/README.md">한국어</a> |
  <a href="https://github.com/JeremyDev87/maximus/blob/master/README.en.md">English</a> |
  <a href="https://github.com/JeremyDev87/maximus/blob/master/README.zh-CN.md">中文</a> |
  <a href="https://github.com/JeremyDev87/maximus/blob/master/README.es.md">Español</a> |
  <a href="https://github.com/JeremyDev87/maximus/blob/master/README.ja.md">日本語</a>
</p>

Pon orden en configuraciones caóticas.

Maximus es una CLI que audita archivos de configuración dispersos por todo un proyecto, ordena conflictos y duplicaciones, y ayuda a los equipos a mantener un entorno de desarrollo limpio y coherente.

Los proyectos modernos se apoyan en muchas capas de configuración como `tsconfig`, `eslint`, `prettier`, `vite`, `jest`, `next.config` y `.env`. Maximus restaura el orden cuando esa configuración empieza a desviarse.

## Transición de Runtime

Maximus ahora prioriza una reescritura en Rust como su dirección de runtime canónico, en lugar de seguir ampliando el backlog de JS como ruta principal de implementación.

- La CLI publicada actualmente y el código ejecutable de este repositorio todavía funcionan sobre Node.js.
- La superficie de comandos para usuarios se mantiene igual: `npx maximus audit`, `npx maximus doctor`, `npx maximus fix`
- Hasta que llegue el cutover, el runtime actual en JS se mantiene como reference implementation.
- `docs/plan/001` hasta `012` ya no deben leerse como tareas directas para ampliar el código JS; ahora son spec inputs para Rust v1.
- `docs/plan/013+` y el backlog JS anterior permanecen en estado deferred hasta que termine el Rust cutover.

Consulta el [documento de runtime transition](https://github.com/JeremyDev87/maximus/blob/master/docs/runtime-transition.md) para ver el límite de la transición, las fases y las reglas para contribuir.

## Qué Hace

- Detecta conflictos de configuración
- Detecta fuentes de configuración duplicadas
- Advierte sobre opciones antiguas de TypeScript
- Revisa alias de rutas mal conectados
- Analiza conflictos entre ESLint y Prettier
- Comprueba variables de entorno faltantes o inconsistentes
- Genera un informe recomendado de estructura del proyecto

## Comandos

```bash
npx maximus audit
npx maximus doctor
npx maximus fix
```

### `audit`

Inspecciona el estado actual de la configuración del proyecto y resume los riesgos principales.

### `doctor`

Un modo de diagnóstico más explicativo que `audit`, con priorización y sugerencias de estructura.

### `fix`

Aplica solo correcciones automáticas seguras.

Correcciones automáticas disponibles en el MVP actual:

- Crear `.env.example` a partir de archivos `.env` reales
- Agregar claves faltantes a `.env.example`

## Ejemplo de Salida

```text
Maximus audit
Target: /workspace/my-app

Status: attention needed
Findings: 1 error, 2 warnings, 1 info
Fixes available: 1

Findings
- [error] Path alias target does not exist
  file: packages/web/tsconfig.json
  detail: @ui/* points to src/missing/*
  hint: Update or remove the stale alias target.

- [warn] Missing .env.example contract
  file: .env
  detail: Runtime env files exist, but .env.example is missing.
  hint: Run `maximus fix` to create a blank contract file.
```

## Desarrollo Local

```bash
npm test
node ./bin/maximus.js audit
node ./bin/maximus.js fix --dry-run
```

Estos comandos locales siguen validando la reference implementation actual en Node.js. Incluso después de iniciar el Rust bootstrap, los ejemplos para usuarios se mantienen con la forma `npx maximus ...`.

## Recomendado Para

- Equipos que operan monorepos o repositorios con múltiples paquetes
- Equipos a los que les cuesta gestionar muchos archivos de configuración
- Equipos donde las nuevas incorporaciones suelen bloquearse durante la configuración inicial

## Contribuir

Las contribuciones son bienvenidas. Si quieres añadir una nueva verificación, mejorar la seguridad de las correcciones automáticas o reducir falsos positivos, empieza por [CONTRIBUTING.md](https://github.com/JeremyDev87/maximus/blob/master/CONTRIBUTING.md) y por el [documento de runtime transition](https://github.com/JeremyDev87/maximus/blob/master/docs/runtime-transition.md), porque la prioridad activa es la familia de reescritura en Rust y no la expansión directa del backlog JS.

## Seguridad

Si crees que encontraste un problema de seguridad, no abras primero un issue público. Usa [SECURITY.md](https://github.com/JeremyDev87/maximus/blob/master/SECURITY.md) para el proceso de reporte privado.

## Sponsor

Si Maximus ayuda a tu equipo a mantener bajo control el caos de configuración, puedes apoyar su mantenimiento mediante [GitHub Sponsors](https://github.com/sponsors/JeremyDev87).

## Licencia

Maximus se distribuye bajo la [MIT License](https://github.com/JeremyDev87/maximus/blob/master/LICENSE).
