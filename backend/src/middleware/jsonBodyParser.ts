import express from "express";

export function jsonBodyParser(limit = process.env.JSON_BODY_LIMIT ?? "100kb") {
  return express.json({ limit });
}
