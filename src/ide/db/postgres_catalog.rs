//! PostgreSQL catalog and reconstructed DDL in the connection's database.
use super::*;
use session::{Catalog, Schema, Object, ObjectKind};
use postgres_sql::{ident, literal};
fn quoted(s: &str) -> String { literal(&Cell::Text(s.into())) }
impl postgres::Connection {
    pub fn catalog(&mut self) -> Result<Catalog, String> {
        let mut catalog = Catalog::default();
        for r in self.query("SELECT nspname FROM pg_catalog.pg_namespace WHERE nspname !~ '^pg_(toast|temp)' ORDER BY nspname IN ('pg_catalog','information_schema'), nspname")?.rows {
            catalog.schemas.push(Schema { name: r[0].text().into(), ..Default::default() });
        }
        for r in self.query("SELECT n.nspname,c.relname,c.relkind,c.oid FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace WHERE c.relkind IN ('r','p','v','m','S') ORDER BY n.nspname,c.relname")?.rows {
            if let Some(s) = catalog.schemas.iter_mut().find(|s| s.name == r[0].text()) {
                if r[2].text() == "S" { s.objects.push(Object { schema: s.name.clone(), name: r[1].text().into(), kind: ObjectKind::Sequence, oid: r[3].text().parse().ok() }); }
                else { s.tables.push(Table { schema: s.name.clone(), name: r[1].text().into(), dialect: Dialect::Postgres, view: matches!(r[2].text(), "v" | "m"), materialized: r[2].text() == "m", ..Default::default() }); }
            }
        }
        for r in self.query("SELECT n.nspname,c.relname,a.attname,pg_catalog.format_type(a.atttypid,a.atttypmod),a.attnotnull,pg_catalog.pg_get_expr(d.adbin,d.adrelid),a.attidentity,a.attgenerated,COALESCE((SELECT ordinality FROM pg_catalog.pg_constraint k CROSS JOIN LATERAL unnest(k.conkey) WITH ORDINALITY x(attnum,ordinality) WHERE k.conrelid=c.oid AND k.contype='p' AND x.attnum=a.attnum),0) FROM pg_catalog.pg_attribute a JOIN pg_catalog.pg_class c ON c.oid=a.attrelid JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace LEFT JOIN pg_catalog.pg_attrdef d ON d.adrelid=c.oid AND d.adnum=a.attnum WHERE a.attnum>0 AND NOT a.attisdropped AND c.relkind IN ('r','p','v','m') ORDER BY n.nspname,c.relname,a.attnum")?.rows {
            if let Some(t) = table_mut(&mut catalog, &r) {
                t.columns.push(Column { name: r[2].text().into(), kind: r[3].text().into(), notnull: r[4].text() == "t", default: (r[5] != Cell::Null).then(|| r[5].text().into()),
                    auto_increment: !r[6].text().is_empty() || r[5].text().starts_with("nextval("), generated: !r[7].text().is_empty() || r[6].text() == "a", pk: r[8].text().parse().unwrap_or(0) });
            }
        }
        for r in self.query("SELECT n.nspname,c.relname,k.conname,pg_catalog.pg_get_constraintdef(k.oid,true) FROM pg_catalog.pg_constraint k JOIN pg_catalog.pg_class c ON c.oid=k.conrelid JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace WHERE k.contype='f' ORDER BY n.nspname,c.relname,k.conname")?.rows {
            if let Some(t) = table_mut(&mut catalog, &r) { t.foreign.push(format!("{}: {}", r[2].text(), r[3].text())); }
        }
        for r in self.query("SELECT n.nspname,c.relname,i.relname,pg_catalog.pg_get_indexdef(i.oid) FROM pg_catalog.pg_index x JOIN pg_catalog.pg_class c ON c.oid=x.indrelid JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace JOIN pg_catalog.pg_class i ON i.oid=x.indexrelid ORDER BY n.nspname,c.relname,i.relname")?.rows {
            if let Some(t) = table_mut(&mut catalog, &r) { t.indexes.push((r[2].text().into(), r[3].text().into())); }
        }
        for r in self.query("SELECT n.nspname,c.relname,t.tgname,t.oid FROM pg_catalog.pg_trigger t JOIN pg_catalog.pg_class c ON c.oid=t.tgrelid JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace WHERE NOT t.tgisinternal ORDER BY n.nspname,c.relname,t.tgname")?.rows {
            if let Some(t) = table_mut(&mut catalog, &r) { t.triggers.push(Object { schema: t.schema.clone(), name: r[2].text().into(), kind: ObjectKind::Trigger, oid: r[3].text().parse().ok() }); }
        }
        for r in self.query("SELECT n.nspname,p.proname || '(' || pg_catalog.pg_get_function_identity_arguments(p.oid) || ')',p.prokind,p.oid FROM pg_catalog.pg_proc p JOIN pg_catalog.pg_namespace n ON n.oid=p.pronamespace WHERE p.prokind IN ('f','p') ORDER BY n.nspname,p.proname,p.oid")?.rows {
            if let Some(s) = catalog.schemas.iter_mut().find(|s| s.name == r[0].text()) {
                s.objects.push(Object { schema: s.name.clone(), name: r[1].text().into(), kind: if r[2].text() == "p" { ObjectKind::Procedure } else { ObjectKind::Function }, oid: r[3].text().parse().ok() });
            }
        }
        Ok(catalog)
    }
    pub fn ddl(&mut self, o: &Object) -> Result<String, String> {
        let target = format!("{}.{}", ident(&o.schema), ident(&o.name));
        let oid = o.oid.map(|id| id.to_string()).unwrap_or_else(|| format!("pg_catalog.to_regclass({})", quoted(&target)));
        match o.kind {
            ObjectKind::Table => {
                let mut parts = vec![];
                for r in self.query(&format!("SELECT a.attname,pg_catalog.format_type(a.atttypid,a.atttypmod),a.attnotnull,a.attidentity,a.attgenerated,pg_catalog.pg_get_expr(d.adbin,d.adrelid) FROM pg_catalog.pg_attribute a LEFT JOIN pg_catalog.pg_attrdef d ON d.adrelid=a.attrelid AND d.adnum=a.attnum WHERE a.attrelid={oid} AND a.attnum>0 AND NOT a.attisdropped ORDER BY a.attnum"))?.rows {
                    let mut col = format!("    {} {}", ident(r[0].text()), r[1].text());
                    if !r[3].text().is_empty() { col += if r[3].text() == "a" { " GENERATED ALWAYS AS IDENTITY" } else { " GENERATED BY DEFAULT AS IDENTITY" }; }
                    else if !r[4].text().is_empty() { col += &format!(" GENERATED ALWAYS AS ({}) {}", r[5].text(), if r[4].text() == "v" { "VIRTUAL" } else { "STORED" }); }
                    else if r[5] != Cell::Null { col += &format!(" DEFAULT {}", r[5].text()); }
                    if r[2].text() == "t" { col += " NOT NULL"; }
                    parts.push(col);
                }
                if parts.is_empty() { return Err("Tabela ausente ou sem colunas.".into()); }
                for r in self.query(&format!("SELECT conname,pg_catalog.pg_get_constraintdef(oid) FROM pg_catalog.pg_constraint WHERE conrelid={oid} AND contype IN ('p','u','f','c','x') ORDER BY conname"))?.rows {
                    parts.push(format!("    CONSTRAINT {} {}", ident(r[0].text()), r[1].text()));
                }
                let mut ddl = format!("CREATE TABLE {target} (\n{}\n);", parts.join(",\n"));
                for r in self.query(&format!("SELECT pg_catalog.pg_get_indexdef(i.indexrelid) FROM pg_catalog.pg_index i WHERE i.indrelid={oid} AND NOT EXISTS (SELECT 1 FROM pg_catalog.pg_constraint k WHERE k.conindid=i.indexrelid AND k.conrelid=i.indrelid) ORDER BY i.indexrelid"))?.rows {
                    ddl += &format!("\n{};", r[0].text());
                }
                Ok(ddl)
            }
            ObjectKind::View | ObjectKind::MaterializedView => {
                let definition = self.definition(&format!("SELECT pg_catalog.pg_get_viewdef({oid},true)"))?;
                Ok(format!("CREATE {} {target} AS\n{definition}", o.kind.sql()))
            }
            ObjectKind::Function | ObjectKind::Procedure | ObjectKind::Trigger | ObjectKind::Index => {
                if matches!(o.kind, ObjectKind::Function | ObjectKind::Procedure | ObjectKind::Trigger) && o.oid.is_none() { return Err("Identificador de catálogo ausente; recarregue a estrutura.".into()); }
                let f = match o.kind { ObjectKind::Trigger => "pg_get_triggerdef", ObjectKind::Index => "pg_get_indexdef", _ => "pg_get_functiondef" };
                let ddl = self.definition(&format!("SELECT pg_catalog.{f}({oid})"))?;
                Ok(if ddl.trim_end().ends_with(';') { ddl } else { format!("{ddl};") })
            }
            ObjectKind::Sequence => {
                let g = self.query(&format!("SELECT pg_catalog.format_type(seqtypid,NULL),seqstart,seqincrement,seqmin,seqmax,seqcache,seqcycle FROM pg_catalog.pg_sequence WHERE seqrelid={oid}"))?;
                let r = g.rows.first().ok_or("Sequência ausente")?;
                Ok(format!("CREATE SEQUENCE {target} AS {} START WITH {} INCREMENT BY {} MINVALUE {} MAXVALUE {} CACHE {} {};", r[0].text(), r[1].text(), r[2].text(), r[3].text(), r[4].text(), r[5].text(), if r[6].text() == "t" { "CYCLE" } else { "NO CYCLE" }))
            }
            ObjectKind::Event => Err("Eventos não disponíveis neste motor.".into()),
        }
    }
    fn definition(&mut self, sql: &str) -> Result<String, String> {
        self.query(sql)?.rows.first().and_then(|r| r.first()).filter(|c| **c != Cell::Null).map(|c| c.text().into()).ok_or("DDL indisponível".into())
    }
}
fn table_mut<'a>(catalog: &'a mut Catalog, row: &[Cell]) -> Option<&'a mut Table> {
    catalog.schemas.iter_mut().find(|s| s.name == row[0].text())?.tables.iter_mut().find(|t| t.name == row[1].text())
}
