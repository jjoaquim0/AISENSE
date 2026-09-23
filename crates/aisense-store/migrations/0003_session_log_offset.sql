-- 0003 — onde cada sessão começa no log do agente (F03-09, aba Logs do inspetor).
-- O log é um arquivo por agente (`logs/<agent_id>.log`) com as sessões em sequência;
-- a posição em bytes no início de cada uma separa "esta sessão" das anteriores.
-- NULL nas sessões gravadas antes desta migração.

ALTER TABLE sessions ADD COLUMN log_offset INTEGER;
