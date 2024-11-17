Web server used to receive Mi temperature sensor data

# How to run

```bash

# Create database
export DATABASE_URL="sqlite:frames.db"
sqlx db create
sqlx migrate run

# Run server
DATABASE_URL="sqlite:frames.db" cargo watch -x run
```

curl -X POST http://0.0.0.0:8080/frame -d '{}' -H 'Content-Type: application/json'
