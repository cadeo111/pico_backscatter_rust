# derived from https://github.com/a5chin/python-uv/blob/5621e2eca428423e7a965bdc188740217e52f3c9/Dockerfile
ARG DEBIAN_VERSION=bookworm
ARG UV_VERSION=0.6
ARG VARIANT=3.12


FROM ghcr.io/astral-sh/uv:$UV_VERSION AS uv

FROM python:$VARIANT-slim-$DEBIAN_VERSION as python_linux_with_packages
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
    gnuradio\
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

FROM python_linux_with_packages

WORKDIR /app

COPY --from=uv /uv /uvx /bin/
COPY pyproject.toml uv.lock ./

ENV PYTHONDONTWRITEBYTECODE=True
ENV PYTHONUNBUFFERED=True
ENV UV_LINK_MODE=copy


RUN uv sync --frozen --no-install-project
