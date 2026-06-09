FROM nginx:alpine3.20
COPY nginx.conf /etc/nginx/nginx.conf
COPY . /usr/share/nginx/html/
EXPOSE 80
CMD ["nginx", "-g", "daemon off;"]
