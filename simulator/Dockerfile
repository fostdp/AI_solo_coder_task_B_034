FROM python:3.12-alpine3.20
WORKDIR /app
COPY profinet_simulator.py /app/profinet_simulator.py
RUN apk add --no-cache tzdata
ENV TZ=Asia/Shanghai
EXPOSE 8081
ENTRYPOINT ["python3", "/app/profinet_simulator.py"]
CMD ["--host", "backend", "--port", "34567", "--electrolyzers", "10", "--sensors", "50", "--interval", "2", "--api-port", "8081"]
