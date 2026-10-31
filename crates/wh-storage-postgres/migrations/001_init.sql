
CREATE SCHEMA IF NOT EXISTS wh;
COMMENT ON SCHEMA wh IS 'Warehouse map storage (wh-storage-postgres)';

CREATE TABLE IF NOT EXISTS wh.warehouses (
    id          BIGINT PRIMARY KEY CHECK (id >= 0),
    name        TEXT             NOT NULL,
    entrance_x  DOUBLE PRECISION NOT NULL,
    entrance_y  DOUBLE PRECISION NOT NULL
);
COMMENT ON TABLE wh.warehouses IS 'Склады / торговые залы';

CREATE TABLE IF NOT EXISTS wh.outline_points (
    warehouse_id BIGINT           NOT NULL REFERENCES wh.warehouses(id) ON DELETE CASCADE,
    idx          INTEGER          NOT NULL CHECK (idx >= 0),
    x            DOUBLE PRECISION NOT NULL,
    y            DOUBLE PRECISION NOT NULL,
    PRIMARY KEY (warehouse_id, idx)
);
COMMENT ON TABLE wh.outline_points IS 'Вершины контура склада (Polygon), idx — порядок обхода';

CREATE TABLE IF NOT EXISTS wh.shelves (
    warehouse_id BIGINT           NOT NULL REFERENCES wh.warehouses(id) ON DELETE CASCADE,
    id           BIGINT           NOT NULL CHECK (id >= 0),
    pos          INTEGER          NOT NULL,
    label        TEXT             NOT NULL,
    cx           DOUBLE PRECISION NOT NULL,
    cy           DOUBLE PRECISION NOT NULL,
    half_w       DOUBLE PRECISION NOT NULL,
    half_d       DOUBLE PRECISION NOT NULL,
    angle        DOUBLE PRECISION NOT NULL,
    double_sided BOOLEAN          NOT NULL,
    capacity     BIGINT           NOT NULL CHECK (capacity >= 0),
    PRIMARY KEY (warehouse_id, id)
);
COMMENT ON TABLE  wh.shelves       IS 'Стеллажи (Shelf + OrientedRect footprint)';
COMMENT ON COLUMN wh.shelves.pos   IS 'Порядок стеллажа в списке склада';
COMMENT ON COLUMN wh.shelves.angle IS 'Поворот footprint, радианы';

CREATE TABLE IF NOT EXISTS wh.products (
    warehouse_id BIGINT NOT NULL REFERENCES wh.warehouses(id) ON DELETE CASCADE,
    id           BIGINT NOT NULL CHECK (id >= 0),
    seq          BIGINT GENERATED ALWAYS AS IDENTITY,
    name         TEXT   NOT NULL,
    size         BIGINT NOT NULL CHECK (size >= 0),
    demand       REAL,
    PRIMARY KEY (warehouse_id, id)
);
COMMENT ON TABLE wh.products IS 'Каталог товаров (по складам)';

CREATE TABLE IF NOT EXISTS wh.placements (
    warehouse_id BIGINT  NOT NULL,
    product_id   BIGINT  NOT NULL,
    shelf_id     BIGINT  NOT NULL,
    pinned       BOOLEAN NOT NULL,
    seq          BIGINT  GENERATED ALWAYS AS IDENTITY,
    PRIMARY KEY (warehouse_id, product_id, shelf_id),
    FOREIGN KEY (warehouse_id, product_id) REFERENCES wh.products(warehouse_id, id) ON DELETE CASCADE,
    FOREIGN KEY (warehouse_id, shelf_id)   REFERENCES wh.shelves(warehouse_id, id)  ON DELETE CASCADE
);
COMMENT ON TABLE wh.placements IS 'Расстановка: какой товар на каком стеллаже; pinned — закреплён вручную';

CREATE OR REPLACE VIEW wh.v_placements AS
SELECT w.name AS warehouse,
       s.label AS shelf,
       p.sku,
       p.name  AS product,
       pl.pinned
FROM wh.placements pl
JOIN wh.warehouses w ON w.id = pl.warehouse_id
JOIN wh.shelves    s ON s.warehouse_id = pl.warehouse_id AND s.id = pl.shelf_id
JOIN wh.products   p ON p.warehouse_id = pl.warehouse_id AND p.id = pl.product_id
ORDER BY w.id, s.pos, pl.seq;
