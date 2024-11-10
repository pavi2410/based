import {createLazyFileRoute} from '@tanstack/react-router'

export const Route = createLazyFileRoute('/project/$id')({
  component: RouteComponent,
})

function RouteComponent() {
  const {id} = Route.useParams()
  return (
    <div className="p-2">
      Hello /project/{id}
    </div>
  )
}
